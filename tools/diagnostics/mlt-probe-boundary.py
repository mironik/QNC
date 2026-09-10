"""Observe stock MLT analysis through FFmpeg's public log API, not code hooks.

This is an isolated qualification tool. It never disables analysis or changes
QNC runtime, media, or DB records. Requires separate explicit test permission.
"""
import argparse
import ctypes as c
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import time


def load_evaluator():
    path = Path(__file__).with_name("mlt-evaluate.py")
    spec = importlib.util.spec_from_file_location("mlt_evaluate", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class AnalysisLog:
    def __init__(self, directory):
        candidates = list(directory.glob("avutil-*.dll")) if os.name == "nt" else list((directory / "lib").glob("libavutil.so.*"))
        if len(candidates) != 1:
            raise RuntimeError("Expected exactly one bundled avutil library")
        self.lib = c.CDLL(str(candidates[0]))
        self.lib.av_log_set_level.argtypes = [c.c_int]
        self.lib.av_log_set_callback.argtypes = [c.c_void_p]
        self.lib.av_log_get_level.restype = c.c_int
        self.old_level = self.lib.av_log_get_level()
        self.events = []
        self.stage = "setup"
        callback_type = c.CFUNCTYPE(None, c.c_void_p, c.c_int, c.c_char_p, c.c_void_p)

        def observe(context, level, format_string, args):
            # No varargs interpretation: count only literal entry/exit markers.
            text = (format_string or b"").decode("utf-8", errors="replace")
            for marker in ("Before avformat_find_stream_info()", "After avformat_find_stream_info()"):
                if marker in text:
                    self.events.append(dict(stage=self.stage, marker=marker, level=level))

        self.callback = callback_type(observe)
        self.lib.av_log_set_callback(c.cast(self.callback, c.c_void_p))
        self.lib.av_log_set_level(48)  # AV_LOG_DEBUG; positive controls are mandatory.

    def count(self, stage):
        return sum(e["stage"] == stage and e["marker"].startswith("Before") for e in self.events)

    def close(self):
        self.lib.av_log_set_callback(c.cast(self.lib.av_log_default_callback, c.c_void_p))
        self.lib.av_log_set_level(self.old_level)


def check(mlt, log, clip, project, kind, service, saved_timing):
    log.events.clear()
    profile = mlt.profile(clip)
    producer = None
    image_hash = None
    try:
        path = clip["video_path"] if kind == "video" else clip["audio_path"]
        log.stage = "construct"
        started = time.perf_counter()
        producer = mlt.factory_producer(profile, service.encode(), str(path).encode("utf-8"))
        if not producer:
            raise RuntimeError("Producer construction failed")
        props = mlt.producer_properties(producer)
        properties = dict(video_index=clip["video_index"] if kind == "video" else -1,
                          audio_index=-1 if kind == "video" else "all", threads=4)
        if saved_timing:
            properties.update(length=clip["frames"], **{"in": 0, "out": clip["frames"]-1},
                              force_fps=clip["fps_num"]/clip["fps_den"],
                              **{"meta.media.frame_rate_num": clip["fps_num"],
                                 "meta.media.frame_rate_den": clip["fps_den"],
                                 "meta.media.width": clip["width"],
                                 "meta.media.height": clip["height"]})
        log.stage = "set_properties"
        for key, value in properties.items():
            if mlt.properties_set(props, key.encode(), str(value).encode()) != 0:
                raise RuntimeError("Could not set MLT property " + key)
        mlt.producer_set_speed(producer, 1.0)
        initial_length = mlt.producer_get_length(producer)
        log.stage = "first_frame"
        frame = mlt.frame(producer, 0)
        try:
            if kind == "video":
                image_hash, _ = mlt.image(frame, clip)
            else:
                raw, _ = mlt.audio(frame, clip, project["audio"]["sample_rate"], len(clip["audio_indices"]))
                image_hash = hashlib.sha256(raw).hexdigest()
        finally:
            mlt.frame_close(frame)
        final_length = mlt.producer_get_length(producer)
        last_hash = None
        if saved_timing and kind == "video":
            log.stage = "last_frame"
            frame = mlt.frame(producer, clip["frames"]-1)
            try:
                last_hash, _ = mlt.image(frame, clip)
            finally:
                mlt.frame_close(frame)
        return dict(clip=clip["name"], kind=kind, service=service, saved_timing=saved_timing,
                    construct_analysis_calls=log.count("construct"),
                    property_analysis_calls=log.count("set_properties"),
                    first_frame_analysis_calls=log.count("first_frame"),
                    last_frame_analysis_calls=log.count("last_frame"),
                    saved_length=clip["frames"], initial_length=initial_length,
                    after_decode_length=final_length,
                    elapsed_ms=round((time.perf_counter()-started)*1000,3),
                    first_data_sha256=image_hash, last_video_sha256=last_hash,
                    analysis_events=list(log.events))
    finally:
        log.stage = "close"
        if producer:
            mlt.producer_close(producer)
        mlt.profile_close(profile)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--mlt-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--allow-stream-analysis", action="store_true")
    args = parser.parse_args()
    if not args.allow_stream_analysis:
        parser.error("This negative test deliberately observes analysis; explicit permission is required")
    root, directory, output = args.root.resolve(), args.mlt_dir.resolve(), args.output.resolve()
    if not output.is_relative_to(root / "target/mlt-eval") or output.suffix != ".json":
        parser.error("Result JSON must stay in target/mlt-eval")
    evaluator = load_evaluator()
    project, clips, databases = evaluator.inputs(root, ["Mironik 2002.MXF", "Mironik 2679.MXF"])
    fingerprint = lambda: {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in databases}
    before = fingerprint()
    mlt = evaluator.Mlt(directory)
    log = AnalysisLog(directory)
    results = dict(engine=mlt.version_get_string().decode(), project=project, cases=[],
                   scope="Isolated local analysis observation; not a no-probe implementation or Ingest live test.")
    try:
        for clip in sorted(clips, key=lambda x:x["name"]):
            for kind in ("video", "original_audio"):
                for service, saved in (("avformat", False), ("avformat-novalidate", False), ("avformat-novalidate", True)):
                    result = check(mlt, log, clip, project, kind, service, saved)
                    results["cases"].append(result)
                    print(json.dumps({k:v for k,v in result.items() if k not in ("analysis_events", "first_data_sha256", "last_video_sha256")}), flush=True)
        for case in results["cases"]:
            before_events = sum(e["marker"].startswith("Before") for e in case["analysis_events"])
            after_events = sum(e["marker"].startswith("After") for e in case["analysis_events"])
            assert before_events == after_events, "Incomplete analysis trace"
            if case["service"] == "avformat":
                assert case["construct_analysis_calls"] > 0, "Positive control did not observe analysis"
            else:
                assert case["construct_analysis_calls"] == 0, "Unexpected eager opening in novalidate"
                assert case["first_frame_analysis_calls"] > 0, "Expected delayed analysis was not observed"
            if case["saved_timing"]:
                assert case["after_decode_length"] == case["saved_length"], "Saved length was overwritten"
        for clip in clips:
            for kind in ("video", "original_audio"):
                matching = [case for case in results["cases"] if case["clip"] == clip["name"] and case["kind"] == kind]
                assert len({case["first_data_sha256"] for case in matching}) == 1, "Options changed the first decoded data"
        results["stock_avformat_meets_no_probe"] = False
        results["positive_controls_passed"] = True
    finally:
        results["db_files_unchanged"] = before == fingerprint()
        log.close()
        mlt.factory_close()
        output.write_text(json.dumps(results, indent=2), encoding="utf-8")
    assert results["db_files_unchanged"], "Observed DB file changed during test"


if __name__ == "__main__":
    main()

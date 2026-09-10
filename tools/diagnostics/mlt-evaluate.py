"""Isolated MLT qualification, NOT a QNC player or a production DB adapter.

Requires explicit permission for MLT's normal stream analysis. Media is read only;
only the caller-selected result JSON is written. Run Python with -I on this host.
"""
import argparse
import ctypes as c
import csv
import hashlib
import json
import os
from pathlib import Path
import random
import sqlite3
import statistics
import subprocess
import time
from urllib.parse import unquote


def read_db(path):
    db = sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)
    db.execute("PRAGMA query_only=ON")
    return db


def evidence(value):
    return value["value"]


def inputs(root, requested):
    registry = root / "data/project_store.db"
    with read_db(registry) as db:
        active = db.execute("SELECT value FROM public_app_settings WHERE key='active_project_id'").fetchone()[0]
        name = db.execute("SELECT name FROM public_projects WHERE project_id=?", (active,)).fetchone()[0]
        directory = db.execute("SELECT local_path FROM project_storage_locations WHERE project_id=?", (active,)).fetchone()[0]
    workspace = Path(directory) / "qnc_project.db"
    with read_db(workspace) as db:
        settings = json.loads(db.execute("SELECT settings_json FROM public_project_settings WHERE project_id=?", (active,)).fetchone()[0])
    config = json.loads((root / "data/ingest-transport.json").read_text(encoding="utf-8"))

    def resolve(uri):
        for source in config["sources"]:
            prefix = source["location"]["uri"] + "/file/"
            if uri.startswith(prefix):
                base = Path(source["location"]["file"]).resolve()
                path = (base / unquote(uri[len(prefix):])).resolve()
                if not path.is_relative_to(base):
                    raise ValueError("Media escaped its transport binding")
                return path
        raise ValueError("No explicit local test binding for " + uri)

    records = root / "data/ingest_media_records.db"
    clips = []
    with read_db(records) as db:
        for (raw,) in db.execute("SELECT s.snapshot_json FROM public_media_heads h JOIN public_media_snapshots s ON s.clip_id=h.clip_id AND s.revision=h.revision WHERE s.phase='final' AND s.completeness='complete'"):
            snapshot = json.loads(raw)
            original = snapshot["metadata"]["original"]
            label = unquote(original["media_uri"]).rsplit("/", 1)[-1]
            if label not in requested:
                continue
            mode = settings["playback"]["input"]
            proxy = snapshot["metadata"].get("proxy")
            if mode == "proxy" and not proxy:
                raise ValueError("Project requires a missing proxy")
            if mode not in ("original", "proxy", "proxy_if_available"):
                raise ValueError("Unknown DB playback input")
            media = proxy if proxy and mode != "original" else original
            video = next(s for s in media["streams"] if s["details"]["kind"] == "video")
            meta = video["details"]["metadata"]
            fps = evidence(meta["frame_rate"])
            count = evidence(meta["frame_count"])
            mode_evidence = evidence(meta["frame_rate_mode"])
            if count["accuracy"] != "exact" or mode_evidence == "variable":
                raise ValueError("This test requires saved exact frame count and no known VFR")
            audio = [s for s in original["streams"] if s["details"]["kind"] == "audio"]
            if any(evidence(s["details"]["metadata"]["channels"]) != 1 for s in audio):
                raise ValueError("This qualification case expects separate original mono streams")
            clips.append(dict(name=label, video_path=resolve(media["media_uri"]),
                              audio_path=resolve(original["media_uri"]), video_uri=media["media_uri"],
                              audio_uri=original["media_uri"], video_index=evidence(video["index"]),
                              audio_indices=[evidence(s["index"]) for s in audio],
                              frames=count["frames"], saved_frame_rate_mode=mode_evidence,
                              fps_num=fps["fps_num"], fps_den=fps["fps_den"],
                              width=evidence(meta["width"]), height=evidence(meta["height"])))
    if sorted(x["name"] for x in clips) != sorted(requested):
        raise ValueError("A requested saved clip is missing or duplicated")
    return dict(project_id=active, project_name=name, playback=settings["playback"], audio=settings["audio"]), clips, [registry, workspace, records]


class Mlt:
    def __init__(self, directory):
        self.dll_dirs = []
        if os.name == "nt":
            self.dll_dirs.append(os.add_dll_directory(str(directory)))
            # MLT's MinGW dlopen uses legacy LoadLibrary, not Python's DLL flags.
            c.windll.kernel32.SetDllDirectoryW(str(directory))
        os.environ["MLT_REPOSITORY"] = str(directory / "lib/mlt")
        os.environ["MLT_DATA"] = str(directory / "share/mlt")
        os.environ["MLT_PROFILES_PATH"] = str(directory / "share/mlt/profiles")
        os.environ["MLT_AVFORMAT_PRODUCER_CACHE"] = "8"
        self.lib = c.CDLL(str(directory / ("libmlt-7.dll" if os.name == "nt" else "lib/libmlt-7.so")))
        self.lib.setenv.argtypes = [c.c_char_p, c.c_char_p, c.c_int]
        for key in ("MLT_REPOSITORY", "MLT_DATA", "MLT_PROFILES_PATH", "MLT_AVFORMAT_PRODUCER_CACHE"):
            self.lib.setenv(key.encode(), os.environ[key].encode(), 1)
        ptr, integer = c.c_void_p, c.c_int
        def bind(name, result, *args):
            fn = getattr(self.lib, "mlt_" + name)
            fn.restype, fn.argtypes = result, args
            setattr(self, name, fn)
        bind("factory_init", ptr, c.c_char_p)
        bind("factory_close", None)
        bind("version_get_string", c.c_char_p)
        bind("profile_load_string", ptr, c.c_char_p)
        bind("profile_close", None, ptr)
        bind("factory_producer", ptr, ptr, c.c_char_p, c.c_char_p)
        bind("producer_properties", ptr, ptr)
        bind("producer_service", ptr, ptr)
        bind("producer_get_length", integer, ptr)
        bind("producer_seek", integer, ptr, integer)
        bind("producer_set_speed", integer, ptr, c.c_double)
        bind("producer_close", None, ptr)
        bind("properties_set", integer, ptr, c.c_char_p, c.c_char_p)
        bind("properties_get_int", integer, ptr, c.c_char_p)
        bind("service_get_frame", integer, ptr, c.POINTER(ptr), integer)
        bind("frame_properties", ptr, ptr)
        bind("frame_get_position", integer, ptr)
        bind("frame_get_image", integer, ptr, c.POINTER(ptr), c.POINTER(integer), c.POINTER(integer), c.POINTER(integer), integer)
        bind("frame_get_audio", integer, ptr, c.POINTER(ptr), c.POINTER(integer), c.POINTER(integer), c.POINTER(integer), c.POINTER(integer))
        bind("image_format_size", integer, integer, integer, integer, c.POINTER(integer))
        bind("frame_close", None, ptr)
        if not self.factory_init(str(directory / "lib/mlt").encode()):
            raise RuntimeError("MLT factory failed")

    def profile(self, clip):
        values = dict(description="QNC isolated saved-media evaluation", frame_rate_num=clip["fps_num"],
                      frame_rate_den=clip["fps_den"], width=clip["width"], height=clip["height"],
                      progressive=1, sample_aspect_num=1, sample_aspect_den=1,
                      display_aspect_num=16, display_aspect_den=9, colorspace=709)
        return self.profile_load_string("\n".join(f"{k}={v}" for k, v in values.items()).encode())

    def producer(self, profile, path, video, audio):
        start = time.perf_counter()
        producer = self.factory_producer(profile, b"avformat", str(path).encode("utf-8"))
        if not producer:
            raise RuntimeError("MLT could not open media")
        props = self.producer_properties(producer)
        for k, v in dict(video_index=video, audio_index=audio, threads=4).items():
            self.properties_set(props, k.encode(), str(v).encode())
        self.producer_set_speed(producer, 1.0)
        return producer, (time.perf_counter() - start) * 1000

    def frame(self, producer, index):
        self.producer_seek(producer, index)
        frame = c.c_void_p()
        err = self.service_get_frame(self.producer_service(producer), c.byref(frame), 0)
        if err or not frame.value or self.frame_get_position(frame) != index:
            raise RuntimeError(f"MLT frame acquisition failed at {index}: {err}")
        return frame

    def image(self, frame, clip):
        ptr, fmt, width, height = c.c_void_p(), c.c_int(4), c.c_int(clip["width"]), c.c_int(clip["height"])
        err = self.frame_get_image(frame, c.byref(ptr), c.byref(fmt), c.byref(width), c.byref(height), 0)
        props = self.frame_properties(frame)
        if err or not ptr.value or self.properties_get_int(props, b"test_image"):
            raise RuntimeError("MLT returned failed/test video")
        size = self.image_format_size(fmt, width, height, None)
        if size <= 0:
            raise RuntimeError("MLT image size invalid")
        digest = hashlib.sha256(c.string_at(ptr, size)).hexdigest()
        return digest, [fmt.value, width.value, height.value]

    def audio(self, frame, clip, rate, channels):
        index = self.frame_get_position(frame)
        samples_count = ((index + 1) * rate * clip["fps_den"] // clip["fps_num"] - index * rate * clip["fps_den"] // clip["fps_num"])
        ptr, fmt, frequency, count, samples = c.c_void_p(), c.c_int(4), c.c_int(rate), c.c_int(channels), c.c_int(samples_count)
        err = self.frame_get_audio(frame, c.byref(ptr), c.byref(fmt), c.byref(frequency), c.byref(count), c.byref(samples))
        props = self.frame_properties(frame)
        if err or not ptr.value or self.properties_get_int(props, b"test_audio"):
            raise RuntimeError("MLT returned failed/test audio")
        if (fmt.value, frequency.value, count.value, samples.value) != (4, rate, channels, samples_count):
            raise RuntimeError(f"Unexpected audio format: {fmt.value, frequency.value, count.value, samples.value}")
        raw = c.string_at(ptr, samples_count * channels * 4)
        return raw, samples_count


def metrics(values):
    ordered = sorted(values)
    return dict(median_ms=round(statistics.median(ordered), 3),
                p95_ms=round(ordered[int((len(ordered)-1)*.95)], 3), max_ms=round(max(ordered), 3))


def check_channels(mlt, clip, project):
    profile = mlt.profile(clip)
    producers = []
    checks = []
    count = len(clip["audio_indices"])
    output_channels = project["audio"]["channels"]
    rate = project["audio"]["sample_rate"]
    if output_channels > count:
        raise ValueError("Project requests more channels than the saved original provides")
    try:
        bundled, _ = mlt.producer(profile, clip["audio_path"], -1, "all")
        producers.append(bundled)
        for stream in clip["audio_indices"][:output_channels]:
            single, _ = mlt.producer(profile, clip["audio_path"], -1, stream)
            producers.append(single)
        for index in [0, 1, clip["frames"]//2, clip["frames"]-2, clip["frames"]-1]:
            frame = mlt.frame(bundled, index)
            try:
                raw, _ = mlt.audio(frame, clip, rate, count)
            finally:
                mlt.frame_close(frame)
            for channel, producer in enumerate(producers[1:]):
                frame = mlt.frame(producer, index)
                try:
                    mono, samples = mlt.audio(frame, clip, rate, 1)
                finally:
                    mlt.frame_close(frame)
                extracted = b"".join(raw[i:i+4] for i in range(channel*4, len(raw), count*4))
                checks.append(dict(frame=index, output_channel=channel+1,
                                   original_stream_index=clip["audio_indices"][channel],
                                   exact_sample_match=extracted==mono, samples=samples))
        return dict(name=clip["name"], project_output_channels=output_channels,
                    original_mono_streams=count, sample_rate=rate, checks=checks,
                    mismatches=[x for x in checks if not x["exact_sample_match"]])
    finally:
        for producer in producers:
            mlt.producer_close(producer)
        mlt.profile_close(profile)


def check_reference(mlt, clip, ffmpeg):
    targets = sorted({0, 1, 24, 25, 26, clip["frames"]//2-1,
                      clip["frames"]//2, clip["frames"]//2+1,
                      clip["frames"]-2, clip["frames"]-1})
    select = "+".join(f"eq(n\\,{index})" for index in targets)
    command = [str(ffmpeg), "-nostdin", "-v", "error", "-threads", "4", "-i", str(clip["video_path"]),
               "-map", f"0:{clip['video_index']}", "-an", "-vf", "select="+select,
               "-fps_mode", "passthrough", "-c:v", "rawvideo", "-pix_fmt", "yuv420p",
               "-f", "framehash", "-hash", "sha256", "-"]
    print("REFERENCE", clip["name"], flush=True)
    run = subprocess.run(command, capture_output=True, text=True, check=True, timeout=300)
    expected_tb = f"#tb 0: {clip['fps_den']}/{clip['fps_num']}"
    if expected_tb not in run.stdout:
        raise ValueError("Reference output time base differs from the saved source")
    rows = csv.reader(line for line in run.stdout.splitlines() if line and not line.startswith("#"))
    reference = {int(row[2]): row[5].strip() for row in rows}
    if sorted(reference) != targets:
        raise ValueError("Reference frame positions differ from the requested positions")
    profile = mlt.profile(clip)
    producer, _ = mlt.producer(profile, clip["video_path"], clip["video_index"], -1)
    checks = []
    try:
        for index in reversed(targets):
            frame = mlt.frame(producer, index)
            try:
                digest, shape = mlt.image(frame, clip)
                checks.append(dict(frame=index, image_shape=shape, match=digest==reference[index],
                                   mlt_sha256=digest, ffmpeg_sha256=reference[index]))
            finally:
                mlt.frame_close(frame)
        return dict(name=clip["name"], checks=checks,
                    mismatches=[x for x in checks if not x["match"]],
                    distinct_reference_frames=len(set(reference.values())),
                    reference_stderr=run.stderr)
    finally:
        mlt.producer_close(producer)
        mlt.profile_close(profile)


def evaluate(mlt, clip, project, limit):
    print("START", clip["name"], flush=True)
    profile = mlt.profile(clip)
    video, open_video = mlt.producer(profile, clip["video_path"], clip["video_index"], -1)
    audio, open_audio = mlt.producer(profile, clip["audio_path"], -1, "all")
    frames = min(clip["frames"], limit or clip["frames"])
    result = {k: v for k,v in clip.items() if not k.endswith("_path")}
    result.update(open_video_ms=round(open_video,3), open_audio_ms=round(open_audio,3),
                  mlt_video_length=mlt.producer_get_length(video), mlt_audio_length=mlt.producer_get_length(audio))
    rate, output_channels = project["audio"]["sample_rate"], project["audio"]["channels"]
    original_channels = len(clip["audio_indices"])
    if output_channels > original_channels:
        raise ValueError("Insufficient original channels for project output")
    hashes, timings, sample_total, shapes = [], [], 0, set()
    start = time.perf_counter()
    try:
        for index in range(frames):
            then = time.perf_counter()
            vf, af = mlt.frame(video, index), mlt.frame(audio, index)
            try:
                vh, shape = mlt.image(vf, clip)
                raw, sample_count = mlt.audio(af, clip, rate, original_channels)
                hashes.append((vh, hashlib.sha256(raw).hexdigest()))
                sample_total += sample_count
                shapes.add(tuple(shape))
            finally:
                mlt.frame_close(vf)
                mlt.frame_close(af)
            timings.append((time.perf_counter() - then)*1000)
            if index % 1000 == 0:
                print("PROGRESS", clip["name"], index, "of", frames, flush=True)
        elapsed = time.perf_counter() - start
        result.update(decoded_frames=frames, elapsed_seconds=round(elapsed,3),
                      decode_fps=round(frames/elapsed,3), image_shapes=sorted(shapes),
                      sample_count_per_original_channel=sample_total, frame_work=metrics(timings),
                      frames_above_20ms=sum(t>20 for t in timings), first_frame_ms=round(timings[0],3))
        # New producers prevent a frame cache from concealing an inaccurate seek.
        mlt.producer_close(video)
        mlt.producer_close(audio)
        video, _ = mlt.producer(profile, clip["video_path"], clip["video_index"], -1)
        audio, _ = mlt.producer(profile, clip["audio_path"], -1, "all")
        targets = [frames-1, 0, 1, frames//2, frames//2-1, frames//2+1, frames-2]
        targets += random.Random(20260909).sample(range(frames), min(frames, 30))
        comparisons = []
        for index in targets:
            then = time.perf_counter()
            vf, af = mlt.frame(video, index), mlt.frame(audio, index)
            try:
                vh, _ = mlt.image(vf, clip)
                raw, _ = mlt.audio(af, clip, rate, original_channels)
                comparisons.append(dict(frame=index, video_match=vh==hashes[index][0],
                                        audio_match=hashlib.sha256(raw).hexdigest()==hashes[index][1],
                                        elapsed_ms=round((time.perf_counter()-then)*1000,3)))
            finally:
                mlt.frame_close(vf)
                mlt.frame_close(af)
        result["seek_checks"] = comparisons
        result["seek_mismatches"] = [x for x in comparisons if not x["video_match"] or not x["audio_match"]]
        return result
    finally:
        mlt.producer_close(video)
        mlt.producer_close(audio)
        mlt.profile_close(profile)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--mlt-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--allow-stream-analysis", action="store_true")
    parser.add_argument("--limit", type=int)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--audio-map-only", action="store_true")
    mode.add_argument("--reference-only", action="store_true")
    args = parser.parse_args()
    if not args.allow_stream_analysis:
        parser.error("Standard MLT analyzes streams; explicit isolated-test permission is required")
    if args.limit is not None and args.limit < 3:
        parser.error("Limit must include at least three frames")
    root = args.root.resolve()
    output = args.output.resolve()
    if not output.is_relative_to(root / "target/mlt-eval"):
        parser.error("Result must stay within QNC target/mlt-eval, never the media or DB directory")
    project, clips, db_paths = inputs(root, ["Mironik 2002.MXF", "Mironik 2679.MXF"])
    fingerprints = lambda: {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in db_paths}
    before = fingerprints()
    mlt = Mlt(args.mlt_dir.resolve())
    results = dict(engine=mlt.version_get_string().decode(), project=project, tests=[],
                   scope="Read-only local isolated decode/seek qualification. Not QNC Ingest, physical A/V output, network or no-probe verification.")
    try:
        for clip in sorted(clips, key=lambda x:x["name"]):
            if args.audio_map_only:
                result = check_channels(mlt, clip, project)
            elif args.reference_only:
                result = check_reference(mlt, clip, args.mlt_dir.resolve() / ("ffmpeg.exe" if os.name == "nt" else "bin/ffmpeg"))
            else:
                result = evaluate(mlt, clip, project, args.limit)
            results["tests"].append(result)
            output.write_text(json.dumps(results, indent=2), encoding="utf-8")
            print("RESULT", json.dumps({k:v for k,v in result.items() if k not in ('seek_checks','checks')}), flush=True)
    finally:
        results["db_files_unchanged"] = before == fingerprints()
        output.write_text(json.dumps(results, indent=2), encoding="utf-8")
        mlt.factory_close()


if __name__ == "__main__":
    main()

// Offline analysis of observations from the actual application, never a player.
import fs from 'node:fs';

const [path, fpsNum, fpsDen] = process.argv.slice(2);
const fps = Number(fpsNum) / Number(fpsDen);
if (!path || !Number.isFinite(fps) || fps <= 0) {
  throw new Error('Usage: node av-offset.mjs LOG SAVED_FPS_NUM SAVED_FPS_DEN');
}
const sessions = new Map();
const get = id => {
  if (!sessions.has(id)) sessions.set(id, {audio: [], video: [], frames: new Map(), transfers: []});
  return sessions.get(id);
};
let status;
for (const line of fs.readFileSync(path, 'utf8').split(/\r?\n/)) {
  let m;
  if ((m = line.match(/AV_A session=(\S+) generation=(\d+) sample=(\d+) rate=(\d+) unix_ns=(\d+)/))) {
    get(m[1]).audio.push({generation: Number(m[2]), seconds: Number(m[3]) / Number(m[4]), time: BigInt(m[5])});
  }
  if ((m = line.match(/AV_F session=(\S+) generation=(\d+) sequence=(\d+) frame=(\d+)/))) {
    const session = get(m[1]);
    const fresh = !session.frames.has(`${m[2]}:${m[3]}`);
    session.frames.set(`${m[2]}:${m[3]}`, Number(m[4]));
    const timing = line.match(/control_us=(\d+) transfer_us=(\d+) total_us=(\d+)/);
    if (timing && fresh) session.transfers.push(timing.slice(1).map(n => Number(n) / 1000));
  }
  if ((m = line.match(/AV_V session=(\S+) generation=(\d+) sequence=(\d+) unix_ns=(\d+)/))) {
    get(m[1]).video.push({key: `${m[2]}:${m[3]}`, time: BigInt(m[4])});
  }
  if (line.startsWith('player-output')) status = line;
}
function stats(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const q = p => sorted[Math.floor((sorted.length - 1) * p)];
  return {n: sorted.length, min: q(0), p05: q(.05), median: q(.5), p95: q(.95), max: q(1)};
}
const results = [];
for (const [id, session] of sessions) {
  const {audio, video, frames} = session;
  audio.sort((a, b) => a.time < b.time ? -1 : a.time > b.time ? 1 : 0);
  const rows = [];
  for (const paint of video) {
    const frame = frames.get(paint.key);
    if (frame === undefined || !audio.length) continue;
    let lo = 0, hi = audio.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (audio[mid].time < paint.time) lo = mid + 1;
      else hi = mid;
    }
    const before = audio[lo - 1], after = audio[lo];
    // Do not interpolate across a seek/pause generation boundary.
    if (before && after && before.generation !== after.generation) continue;
    const near = [before, after].filter(Boolean)
      .sort((a, b) => Math.abs(Number(paint.time - a.time)) - Math.abs(Number(paint.time - b.time)))[0];
    const delta = Number(paint.time - near.time) / 1e9;
    if (Math.abs(delta) <= .075) {
      rows.push({frame, offset: (near.seconds + delta - frame / fps) * 1000});
    }
  }
  results.push({session: id, audio_observations: audio.length, first_frame: rows[0]?.frame,
    last_frame: rows.at(-1)?.frame, offset_ms: stats(rows.map(r => r.offset)),
    after_5_seconds_ms: stats(rows.filter(r => r.frame / fps >= 5).map(r => r.offset)),
    control_ms: stats(session.transfers.map(r => r[0])),
    transfer_ms: stats(session.transfers.map(r => r[1])),
    receive_and_decode_ms: stats(session.transfers.map(r => r[2]))});
}
console.log(JSON.stringify({definition: 'positive = audio ahead of CPU paint; not physical A/V calibration',
  fps, sessions: results, last_status: status}, null, 2));

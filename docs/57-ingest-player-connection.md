# Ingest player connection

Scope approved by the user: connect the existing keyboard play_pause and
Ingest monitor to the public Broadcast Player process for a live test.

The v4 monitor is a passive picture surface; controls send actions to the
player. Keep the existing Ingest rectangles, labels, keyboard catalog and
font. No new controls or layout changes. Click displays the selected thumbnail
while the player prepares. Only confirmed Play switches to actual submitted
video; Pause holds that video picture. A new selection returns to its thumbnail.
The timeline remains a passive view.

Selection prepares one isolated player session off the UI thread, using the
existing read-only SettingsReader -> InputReader path. Play never loads the
DB or opens a decoder. A reusable client owns process/transport lifecycle,
not a second playback clock. Source changes invalidate old replies/frames.
Clicking a different clip always cuts the preceding player session, including
when the new record is pending/invalid. The old process is shut down and
reaped before a new process is prepared. New selections never inherit Play.

The process gains an offscreen monitor mode: already-converted RGBA frames
are delivered over an authenticated, bounded binary HTTP endpoint. No second
decoder, GPU readback, probe, image files or shared-memory-only channel.
Audio is rendered by the player on the workstation hosting that process;
the initial monitor routing explicitly selects the first two native channels
(one if mono), without downmixing or relabelling the source as stereo.
Local, LAN and Intranet media access retain the existing URI/binding path.
Remote audio-device streaming is not implemented or claimed by this step.

Project code/settings, card contents, shortcuts and generation workflows
are outside this change. Verification results are appended after testing.

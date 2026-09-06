# Workstation identity and project origin

Date: 2026-09-06. User-approved scope: public workstation identity module,
Project integration, metadata persistence, only project name/date visible.

## Boundaries

`qnc-workstation-identity` is a stateless public module. It has no application,
database, UI, active-project, authentication, session-routing or network client
dependency. `read_local_identity()` reads the current workstation name, local
OS account name, and hardware serial. It returns a versioned serde JSON DTO.
The standalone `qnc-workstation-identity [--json]` helper prints the same DTO
and exits; partial/unavailable fields are null with explicit issues, not made
up values. Unsupported command-line arguments exit 2. Output failure exits 1.

The helper must run on the originating workstation, not the DB server. This
works offline and does not depend on Project, Ingest or shell being installed.
A future LAN/intranet DB transport stores the caller's snapshot unchanged.
This change does NOT implement network DB writes, project transfer, concurrent
session selection, remote discovery or authenticated network user identity.
The local account name is provenance, not a globally unique or trusted user ID.

## Data contract

`IdentitySnapshot` version 0.1.0 contains:

- `contract_version`;
- `workstation_name`: local OS host name, or null;
- `user_name`: local OS account name, or null;
- `hardware_serial`: null, or `{kind, value, source}`;
- `issues`: `{field, source, reason}` for failed/unavailable reads.

Device serial takes priority over processor serial. Serial source is preserved.
OEM placeholders, empty values, invalid encodings/control characters and
all-zero/all-F values are rejected. Case of a real serial is preserved.
CPUID/ProcessorId, model, MAC address, disk serial and generated UUID are never
substituted for a manufacturer serial. This metadata is NOT an authentication
credential or a guarantee that hardware/VM serials are globally unique.

Windows uses `smbios-lib` with native firmware access, Type 1 device serial or
Type 4 processor Serial Number (not ProcessorId). Linux first reads DMI system
serial or device-tree serial, then firmware records when permitted. macOS reads
IOKit `IOPlatformSerialNumber`, on Intel and Apple Silicon. No elevation,
external shell commands, media access or network access is used by the module.
OS-specific filesystem paths exist only inside the Linux provider, never in
the public snapshot. `hostname` and `whoami` provide native OS name/account
reads. Availability depends on OS permissions and manufacturer firmware.

## Project persistence

Project calls the module once per new project, before entering DB write
transactions. The owner writes the same immutable snapshot into
`project_origin` in both the registry and the individual project DB. Each row
contains project ID, original project name, creation timestamp and identity
JSON. `public_project_origin` exposes those fields and normalized read columns
without requiring any application code to consume them. Hardware metadata is
not added to template settings. Reopen does not recapture or rewrite origin.
Delete removes the corresponding registry origin row through the owner store.

New project IDs contain a random UUID v4, independent of name, clock, serial,
OS path or machine. Template ID generation is unchanged. DB schema contracts
are 0.2.0. There is no data migration, inferred origin or backfill. The user
cleared the previous registry project records; old unregistered directories
remain untouched. Existing files are not repopulated or imported.

## Approved UI difference

Before implementation: user explicitly requests only project name and date
visible. Preserve existing Project board, row height, selection/open action,
X delete control and dialogs. Add creation date to the existing project row,
reserving its width so a long name cannot push it outside the row. Name may
truncate. No workstation, account, serial, UUID or diagnostic field is shown
in the form, hover text or status. Date is DD.MM.YYYY. in local display time;
stored timestamp remains UTC epoch in the existing `epoch_SECONDS` format.
The owner supplies the formatted date; the form only paints it. The same row
renderer is used standalone and shell-hosted.

## Verification

Completed on 2026-09-06:

- `cargo test --workspace --locked --quiet`: 164 tests passed.
- Targeted owner/module tests: identical snapshots in both DBs, one creation
  timestamp, immutable origin on reopen, portable DB public view, no backfill,
  UUID v4 uniqueness, deletion of the matching origin row, explicit null serial.
- Module tests: device/processor priority, OEM placeholder rejection, genuine
  Type 4 serial instead of CPUID, short firmware record, JSON round trip,
  module boundaries, standalone helper outside QNC, invalid CLI argument.
- `cargo clippy -p qnc-workstation-identity --all-targets --locked -- -D warnings`:
  passed. Workspace conformance passed with the absolute QNC root.
- Native Windows read matched host/account and the system serial reported by
  `Win32_ComputerSystemProduct`. Only comparison booleans were reported; actual
  hardware/account values were not committed to code or documentation.
- Read-only inspection of newly created real development projects confirmed
  station, user and device serial in the same JSON snapshot in both databases.
- Windows standalone maximized Project and shell-hosted Project in the narrower
  window were visually inspected. Rows show name and date only; metadata is not
  rendered. Existing board/actions remain in place. No full v4 pixel-diff or
  long-name live test was performed in this change.
- Module compilation passed for x86_64-pc-windows-msvc (native build),
  aarch64-pc-windows-msvc, x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu,
  x86_64-apple-darwin and aarch64-apple-darwin.

Physical Linux/macOS/ARM hardware reads and LAN/intranet end-to-end operation
were NOT tested. Cross-compilation is not runtime verification. Network DB
transport and session selection remain outside this implementation. Project
was re-frozen after completing this approved scope. New projects created by
the user during live testing were preserved; old unregistered directories
were not deleted or reimported.

## Sources

- [Windows firmware API](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemfirmwaretable)
- [Processor serial versus ProcessorId](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-processor)
- [SMBIOS parser](https://github.com/jrgerber/smbios-lib)
- [Linux DMI fields](https://github.com/torvalds/linux/blob/master/drivers/firmware/dmi-id.c)
- [Apple IOKit API](https://github.com/apple-oss-distributions/IOKitUser/blob/main/IOKitLib.h)

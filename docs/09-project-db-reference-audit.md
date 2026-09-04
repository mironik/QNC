# Project DB reference audit

Status: qnc_v4 schema snapshot  
Datum: 2026-09-04

## Izvori

Pregledani qnc_v4 izvori:

```text
qnc-host/src/project/db.rs
qnc-host/src/project/store.rs
qnc-host/src/project/templates.rs
qnc-host/src/project/ui_state.rs
qnc-host/src/project/keyboard_settings.rs
qnc-host/src/project/db_broker.rs
```

## Zakljucak

Project je vlasnik dvije razine baze:

```text
globalno:
  data/project_store.db

po projektu:
  projects/{project_id}/qnc_project.db
```

Ove fizicke putanje su implementacijski detalj resolvera. Javni contract koristi
QNC URI:

```text
qnc://local/db/project_registry
qnc://local/db/project_workspace/{project_id}
```

## Globalna Project registry baza

Izvor u qnc_v4: `init_global_schema`.

Tablice:

```text
projects
app_settings
users
sessions
source_templates
project_templates
module_state
local_runtime_settings
project_storage_locations
project_template_kv
project_template_sources
source_template_kv
```

Vlasnik pisanja: `qnc.project`.

Javni read contract:

```text
public_projects
public_app_settings
public_project_templates
public_source_templates
public_module_state
```

`local_runtime_settings` i `project_storage_locations` su privatne Project
runtime tablice. One smiju sadrzavati lokalne filesystem detalje potrebne za
resolver i kreiranje foldera, ali se ne smiju koristiti kao javni identitet
projekta. Javni identitet projekta ostaje `qnc://local/project/{project_id}`.

## Project workspace baza

Izvor u qnc_v4: `init_project_schema`.

Tablice:

```text
project_settings
project_members
project_template_snapshot
project_workflow_steps
project_workflow_state
project_data_revisions
project_settings_kv
project_snapshot_kv
project_workflow_step_kv
```

Vlasnik pisanja: `qnc.project`.

Javni read contract:

```text
public_project_settings
public_project_members
public_project_template_snapshot
public_project_workflow_steps
public_project_workflow_state
public_project_data_revisions
```

## Pravilo za novi QNC

Project aplikacija jedina smije pisati ove baze i raditi migracije nad njima.
Druge aplikacije smiju citati samo javne viewove kroz DB contract i transport
resolver.

Project UI ne smije direktno pisati SQLite. UI salje neutralni intent Project
aplikaciji, a Project aplikacija kao owner odlucuje o DB upisu.

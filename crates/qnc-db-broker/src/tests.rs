use super::*;
use std::sync::Arc;

const URI: &str = "qnc://local/db/project_db/p1";

/// A toy table module: one counter table it owns.
struct Counter;
struct CounterModule(rusqlite::Connection);

impl TableModuleFactory for Counter {
    fn id(&self) -> &'static str {
        "test.counter"
    }
    fn is_write(&self, payload: &Value) -> bool {
        payload == "add"
    }
    fn attach(&self, conn: rusqlite::Connection, access: Access) -> Result<Box<dyn TableModule>> {
        if access == Access::ReadWrite {
            conn.execute_batch("CREATE TABLE IF NOT EXISTS counter (n INTEGER)")
                .map_err(|e| e.to_string())?;
        }
        Ok(Box::new(CounterModule(conn)))
    }
}

impl TableModule for CounterModule {
    fn execute(&mut self, payload: Value) -> Result<Value> {
        if payload == "add" {
            let tx = self
                .0
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO counter VALUES (1)", [])
                .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        }
        let n: i64 = self
            .0
            .query_row("SELECT count(*) FROM counter", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        Ok(Value::from(n))
    }
}

fn project(dir: &std::path::Path, id: &str) -> std::path::PathBuf {
    let file = dir.join("project.db");
    rusqlite::Connection::open(&file)
        .unwrap()
        .execute_batch(&format!(
            "CREATE TABLE project_settings (project_id TEXT);
             INSERT INTO project_settings VALUES ('{id}');
             CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;"
        ))
        .unwrap();
    file
}

fn modules() -> Vec<Arc<dyn TableModuleFactory>> {
    vec![Arc::new(Counter)]
}

#[test]
fn the_intermediary_serves_the_modules_it_is_given_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let file = project(dir.path(), "p1");
    let target = ProjectDbTarget::from_owner_binding(&file, URI).unwrap();
    let writer = ProjectDbWriter::start(target.clone(), modules()).unwrap();
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let writer = writer.clone();
            std::thread::spawn(move || {
                for _ in 0..5 {
                    writer.call("test.counter", Value::from("add")).unwrap();
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let n: i64 = writer.request("test.counter", &"count").unwrap();
    assert_eq!(n, 20);
    let error = writer.call("test.unknown", Value::Null).unwrap_err();
    assert!(error.contains("Nepoznat modul"), "{error}");
    let mut reader = target.open(Access::ReadOnly, modules()).unwrap();
    assert_eq!(reader.execute("test.counter", Value::from("count")).unwrap(), 20);
    assert!(reader.execute("test.counter", Value::from("add")).is_err(), "read-only");
}

#[test]
fn only_the_database_of_that_project_is_opened_and_never_created() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("none.db");
    let target = ProjectDbTarget::from_owner_binding(&missing, URI).unwrap();
    assert!(target.open(Access::ReadWrite, modules()).is_err());
    assert!(!missing.exists(), "nothing creates a project database");
    let file = project(dir.path(), "other");
    let target = ProjectDbTarget::from_owner_binding(&file, URI).unwrap();
    let error = target.open(Access::ReadWrite, modules()).err().unwrap();
    assert!(error.contains("drugom projektu"), "{error}");
    let plain = dir.path().join("plain.db");
    rusqlite::Connection::open(&plain)
        .unwrap()
        .execute_batch("CREATE TABLE x (a)")
        .unwrap();
    let target = ProjectDbTarget::from_owner_binding(&plain, URI).unwrap();
    assert!(target.open(Access::ReadWrite, modules()).is_err());
}

#[test]
fn uris_come_from_the_workspace_of_the_project() {
    assert_eq!(
        project_db_uri("qnc://local/db/project_workspace/p1").unwrap(),
        "qnc://local/db/project_db/p1"
    );
    assert_eq!(project_id(URI).unwrap(), "p1");
    assert!(project_id("qnc://local/db/ingest_content/p1").is_err());
    assert!(project_db_uri("qnc://local/db/other/p1").is_err());
}

/// An idle writer lets the project database go, so a closed project can be deleted while
/// the application keeps its writer; the next request opens it again.
#[test]
fn an_idle_writer_releases_the_project_database_and_reopens_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = project(dir.path(), "p1");
    let target = ProjectDbTarget::from_owner_binding(&file, URI).unwrap();
    let writer = ProjectDbWriter::start(target, modules()).unwrap();
    writer.call("test.counter", Value::from("add")).unwrap();
    std::thread::sleep(crate::transport::IDLE_RELEASE + std::time::Duration::from_millis(500));
    let moved = dir.path().join("moved.db");
    std::fs::rename(&file, &moved).expect("an idle writer holds no handle");
    std::fs::rename(&moved, &file).unwrap();
    assert_eq!(writer.call("test.counter", Value::from("add")).unwrap(), Value::from(2));
}

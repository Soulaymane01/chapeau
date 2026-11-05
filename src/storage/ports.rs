use crate::errors::Result;
use crate::model::{PortProtocol, ServicePort};
use rusqlite::{params, Connection};
use std::str::FromStr;

/// Replace all ports recorded for an item.
///
/// The caller is responsible for wrapping this in a transaction when
/// consistency with other writes matters (the scan does).
pub fn replace_for_item(conn: &Connection, item_id: &str, ports: &[ServicePort]) -> Result<()> {
    conn.execute(
        "DELETE FROM service_ports WHERE item_id = ?1",
        params![item_id],
    )?;

    for port in ports {
        conn.execute(
            "INSERT OR REPLACE INTO service_ports
                 (item_id, protocol, address, port, pid, process_name, source, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                port.item_id,
                port.protocol.as_str(),
                port.address,
                i64::from(port.port),
                port.pid.map(i64::from),
                port.process_name,
                port.source,
                port.last_seen,
            ],
        )?;
    }
    Ok(())
}

/// List the ports recorded for one item.
pub fn list_for_item(conn: &Connection, item_id: &str) -> Result<Vec<ServicePort>> {
    let mut stmt = conn.prepare(
        "SELECT item_id, protocol, address, port, pid, process_name, source, last_seen
         FROM service_ports
         WHERE item_id = ?1
         ORDER BY port, protocol",
    )?;
    let rows = stmt.query_map(params![item_id], row_to_port)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// List every recorded service port.
pub fn list_all(conn: &Connection) -> Result<Vec<ServicePort>> {
    let mut stmt = conn.prepare(
        "SELECT item_id, protocol, address, port, pid, process_name, source, last_seen
         FROM service_ports
         ORDER BY port, protocol, item_id",
    )?;
    let rows = stmt.query_map([], row_to_port)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

/// Delete all ports for an item.
pub fn delete_for_item(conn: &Connection, item_id: &str) -> Result<usize> {
    let rows = conn.execute(
        "DELETE FROM service_ports WHERE item_id = ?1",
        params![item_id],
    )?;
    Ok(rows)
}

fn row_to_port(row: &rusqlite::Row<'_>) -> rusqlite::Result<ServicePort> {
    let protocol_str: String = row.get(1)?;
    let protocol = PortProtocol::from_str(&protocol_str)
        .map_err(|_| rusqlite::Error::InvalidParameterName(protocol_str.clone()))?;
    let port: i64 = row.get(3)?;
    let port = u16::try_from(port)
        .map_err(|_| rusqlite::Error::InvalidParameterName(format!("invalid port {port}")))?;
    let pid: Option<i64> = row.get(4)?;

    Ok(ServicePort {
        item_id: row.get(0)?,
        protocol,
        address: row.get(2)?,
        port,
        pid: pid.and_then(|value| u32::try_from(value).ok()),
        process_name: row.get(5)?,
        source: row.get(6)?,
        last_seen: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ItemKind, ItemSource};
    use crate::storage::{items, Database};

    fn temp_db() -> Database {
        Database::open_memory().expect("failed to create in-memory database")
    }

    fn service(db: &Database, name: &str, key: &str) -> String {
        items::upsert(
            db.conn(),
            items::NewItem {
                kind: ItemKind::Service,
                name,
                source: ItemSource::Systemd,
                source_key: key,
                scope: Some(crate::model::Scope::System),
                version: None,
                description: None,
                icon: None,
                command: None,
                metadata: None,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn replace_is_idempotent() {
        let db = temp_db();
        let id = service(&db, "PostgreSQL", "postgresql.service");
        let first = vec![
            ServicePort::new(&id, PortProtocol::Tcp, "127.0.0.1", 5432, "ss"),
            ServicePort::new(&id, PortProtocol::Tcp, "::1", 5432, "ss"),
        ];
        replace_for_item(db.conn(), &id, &first).unwrap();
        assert_eq!(list_for_item(db.conn(), &id).unwrap().len(), 2);

        let second = vec![ServicePort::new(
            &id,
            PortProtocol::Tcp,
            "0.0.0.0",
            5432,
            "socket",
        )];
        replace_for_item(db.conn(), &id, &second).unwrap();
        let stored = list_for_item(db.conn(), &id).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].address, "0.0.0.0");
        assert_eq!(stored[0].source, "socket");
    }

    #[test]
    fn ports_include_process_details() {
        let db = temp_db();
        let id = service(&db, "MariaDB", "mariadb.service");
        let mut port = ServicePort::new(&id, PortProtocol::Tcp, "0.0.0.0", 3306, "ss");
        port.pid = Some(1234);
        port.process_name = Some("mariadbd".into());
        replace_for_item(db.conn(), &id, &[port.clone()]).unwrap();

        let stored = list_for_item(db.conn(), &id).unwrap();
        assert_eq!(stored, vec![port]);
        assert_eq!(list_all(db.conn()).unwrap().len(), 1);
    }

    #[test]
    fn udp_ports_are_supported() {
        let db = temp_db();
        let id = service(&db, "DNS", "dns.service");
        let port = ServicePort::new(&id, PortProtocol::Udp, "0.0.0.0", 53, "ss");
        replace_for_item(db.conn(), &id, &[port]).unwrap();

        let stored = list_for_item(db.conn(), &id).unwrap();
        assert_eq!(stored[0].protocol, PortProtocol::Udp);
    }

    #[test]
    fn deleting_an_item_cascades_ports() {
        let db = temp_db();
        let id = service(&db, "Redis", "redis.service");
        replace_for_item(
            db.conn(),
            &id,
            &[ServicePort::new(
                &id,
                PortProtocol::Tcp,
                "127.0.0.1",
                6379,
                "ss",
            )],
        )
        .unwrap();

        items::delete(db.conn(), &id).unwrap();
        assert!(list_for_item(db.conn(), &id).unwrap().is_empty());
    }
}

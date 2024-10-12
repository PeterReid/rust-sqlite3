extern crate sqlite3;



use sqlite3::{
    DatabaseConnection,
    Query,
    ResultRowAccess,
    SqliteResult,
    StatementUpdate,
};

#[derive(Debug)]
struct Person {
    id: i32,
    name: String,
    //time_created: Timespec,
    // TODO: data: Option<Vec<u8>>
}

pub fn main() {
    match io() {
        Ok(ppl) => println!("Found people: {:?}", ppl),
        Err(oops) => panic!("{:?}", oops)
    }
}

fn io() -> SqliteResult<Vec<Person>> {
    let mut conn = try!(DatabaseConnection::in_memory());

    conn.exec("CREATE TABLE person (
                 id              SERIAL PRIMARY KEY,
                 name            VARCHAR NOT NULL
               )")?;

    let me = Person {
        id: 0,
        name: format!("Dan")
    };
    {
        let mut tx = conn.prepare("INSERT INTO person (name)
                           VALUES ($1)")?;
        let changes = tx.update(&[&me.name])?;
        assert_eq!(changes, 1);
    };

    let mut stmt = conn.prepare("SELECT id, name FROM person")?;

    let mut ppl = vec!();
    try!(stmt.query(
        &[], &mut |row| {
            ppl.push(Person {
                id: row.get("id"),
                name: row.get("name")
            });
            Ok(())
        }));
    Ok(ppl)
}

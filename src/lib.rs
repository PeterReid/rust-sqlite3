//! `rust-sqlite3` is a rustic binding to the [sqlite3 API][].
//!
//! [sqlite3 API]: http://www.sqlite.org/c3ref/intro.html
//!
//! Three layers of API are provided:
//!
//!  - `mod core` provides a minimal safe interface to the basic sqlite3 API.
//!  - `mod types` provides `ToSql`/`FromSql` traits, and the library provides
//!     convenient `query()` and `update()` APIs.
//!
//! [bindgen]: https://github.com/crabtw/rust-bindgen
//!
//! The following example demonstrates opening a database, executing
//! DDL, and using the high-level `query()` and `update()` API. Note the
//! use of `Result` and `?` for error handling.
//!
//! ```rust
//! extern crate sqlite3;
//! 
//! 
//! 
//! use sqlite3::{
//!     DatabaseConnection,
//!     Query,
//!     ResultRowAccess,
//!     SqliteResult,
//!     StatementUpdate,
//! };
//! 
//! #[derive(Debug)]
//! struct Person {
//!     id: i32,
//!     name: String,
//!     // TODO: data: Option<Vec<u8>>
//! }
//! 
//! pub fn main() {
//!     match io() {
//!         Ok(ppl) => println!("Found people: {:?}", ppl),
//!         Err(oops) => panic!("{}", oops)
//!     }
//! }
//! 
//! fn io() -> SqliteResult<Vec<Person>> {
//!     let mut conn = DatabaseConnection::in_memory()?;
//! 
//!     conn.exec("CREATE TABLE person (
//!                  id              SERIAL PRIMARY KEY,
//!                  name            VARCHAR NOT NULL
//!                )")?;
//! 
//!     let me = Person {
//!         id: 0,
//!         name: format!("Dan"),
//!     };
//!     {
//!         let mut tx = conn.prepare("INSERT INTO person (name)
//!                            VALUES ($1)")?;
//!         let changes = tx.update(&[&me.name])?;
//!         assert_eq!(changes, 1);
//!     }
//! 
//!     let mut stmt = conn.prepare("SELECT id, name FROM person")?;
//! 
//!     let mut ppl = vec!();
//!     stmt.query(
//!         &[], &mut |row| {
//!             ppl.push(Person {
//!                 id: row.get("id"),
//!                 name: row.get("name")
//!             });
//!             Ok(())
//!         })?;
//!     Ok(ppl)
//! }
//! ```

#![crate_name = "sqlite3"]
#![crate_type = "lib"]
#![warn(missing_docs)]

extern crate libc;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate enum_primitive;

use std::error::{Error};
use std::fmt::Display;
use std::fmt;

pub use core::Access;
pub use core::{DatabaseConnection, PreparedStatement, ResultSet, ResultRow, Value, Context};
pub use core::{ColIx, ParamIx};
pub use types::{FromSql, ToSql};

use self::SqliteErrorCode::SQLITE_MISUSE;

pub mod core;
pub mod types;

/// bindgen-bindings to libsqlite3
#[allow(non_camel_case_types, non_snake_case)]
#[allow(dead_code)]
#[allow(missing_docs)]
#[allow(missing_copy_implementations)]  // until I figure out rust-bindgen #89

pub mod access;

/// Mix in `update()` convenience function.
pub trait StatementUpdate {
    /// Execute a statement after binding any parameters.
    fn update(&mut self,
              values: &[&dyn ToSql]) -> SqliteResult<u64>;
}


impl StatementUpdate for core::PreparedStatement {
    /// Execute a statement after binding any parameters.
    ///
    /// When the statement is done, The [number of rows
    /// modified][changes] is reported.
    ///
    /// Fail with `Err(SQLITE_MISUSE)` in case the statement results
    /// in any any rows (e.g. a `SELECT` rather than `INSERT` or
    /// `UPDATE`).
    ///
    /// [changes]: http://www.sqlite.org/c3ref/changes.html
    fn update(&mut self,
              values: &[&dyn ToSql]) -> SqliteResult<u64> {
        let check = {
            bind_values(self, values)?;
            let mut results = self.execute();
            match results.step()? {
                None => Ok(()),
                Some(_row) => Err(SqliteError {
                    kind: SQLITE_MISUSE,
                    desc: "unexpected SQLITE_ROW from update",
                    detail: None
                })
            }
        };
        check.map(|_ok| self.changes())
    }
}


/// Mix in `query()` convenience function.
pub trait Query<F>
    where F: FnMut(&mut ResultRow) -> SqliteResult<()>
{
    /// Process rows from a query after binding parameters.
    fn query(&mut self,
             values: &[&dyn ToSql],
             each_row: &mut F
             ) -> SqliteResult<()>;
}

impl<F> Query<F> for core::PreparedStatement
    where F: FnMut(&mut ResultRow) -> SqliteResult<()>
{
    /// Process rows from a query after binding parameters.
    ///
    /// For call `each_row(row)` for each resulting step,
    /// exiting on `Err`.
    fn query(&mut self,
             values: &[&dyn ToSql],
             each_row: &mut F
             ) -> SqliteResult<()>
    {
        bind_values(self, values)?;
        let mut results = self.execute();
        loop {
            match results.step()? {
                None => break,
                Some(ref mut row) => each_row(row)?,
            }
        }
        Ok(())
    }
}

fn bind_values(s: &mut PreparedStatement, values: &[&dyn ToSql]) -> SqliteResult<()> {
    for (ix, v) in values.iter().enumerate() {
        let p = ix as ParamIx + 1;
        v.to_sql(s, p)?;
    }
    Ok(())
}


/// Access result columns of a row by name or numeric index.
pub trait ResultRowAccess {
    /// Get `T` type result value from `idx`th column of a row.
    ///
    /// # Panic
    ///
    /// Panics if there is no such column or value.
    fn get<'a, I: RowIndex + Display + Clone, T: FromSql<'a>>(&'a mut self, idx: I) -> T;

    /// Try to get `T` type result value from `idx`th column of a row.
    fn get_opt<'a, I: RowIndex + Display + Clone, T: FromSql<'a>>(&'a mut self, idx: I) -> SqliteResult<T>;
}

impl<'res, 'row> ResultRowAccess for core::ResultRow<'res, 'row> {
    fn get<'a, I: RowIndex + Display + Clone, T: FromSql<'a>>(&'a mut self, idx: I) -> T {
        match self.get_opt(idx.clone()) {
            Ok(ok) => ok,
            Err(err) => panic!("retrieving column {}: {}", idx, err)
        }
    }

    fn get_opt<'a, I: RowIndex + Display + Clone, T: FromSql<'a>>(&'a mut self, idx: I) -> SqliteResult<T> {
        match idx.idx(self) {
            Some(idx) => FromSql::from_sql(self, idx),
            None => Err(SqliteError {
                kind: SQLITE_MISUSE,
                desc: "no such row name/number",
                detail: Some(format!("{}", idx))
            })
        }
    }

}

/// A trait implemented by types that can index into columns of a row.
///
/// *inspired by sfackler's [RowIndex][]*
/// [RowIndex]: http://www.rust-ci.org/sfackler/rust-postgres/doc/postgres/trait.RowIndex.html
pub trait RowIndex {
    /// Try to convert `self` to an index into a row.
    fn idx(&self, row: &mut ResultRow) -> Option<ColIx>;
}

impl RowIndex for ColIx {
    /// Index into a row directly by uint.
    fn idx(&self, _row: &mut ResultRow) -> Option<ColIx> { Some(*self) }
}

impl RowIndex for &'static str {
    /// Index into a row by column name.
    ///
    /// *TODO: figure out how to use lifetime of row rather than
    /// `static`.*
    fn idx(&self, row: &mut ResultRow) -> Option<ColIx> {
        let mut ixs = 0 .. row.column_count();
        ixs.find(|ix| row.with_column_name(*ix, false, |name| name == *self))
    }
}


/// The type used for returning and propagating sqlite3 errors.
#[must_use]
pub type SqliteResult<T> = Result<T, SqliteError>;

/// Result codes for errors.
///
/// cf. [sqlite3 result codes][codes].
///
/// Note `SQLITE_OK` is not included; we use `Ok(...)` instead.
///
/// Likewise, in place of `SQLITE_ROW` and `SQLITE_DONE`, we return
/// `Some(...)` or `None` from `ResultSet::next()`.
///
/// [codes]: http://www.sqlite.org/c3ref/c_abort.html
enum_from_primitive! {
    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    #[allow(non_camel_case_types)]
    #[allow(missing_docs)]
    pub enum SqliteErrorCode {
        SQLITE_ERROR     =  1,
        SQLITE_INTERNAL  =  2,
        SQLITE_PERM      =  3,
        SQLITE_ABORT     =  4,
        SQLITE_BUSY      =  5,
        SQLITE_LOCKED    =  6,
        SQLITE_NOMEM     =  7,
        SQLITE_READONLY  =  8,
        SQLITE_INTERRUPT =  9,
        SQLITE_IOERR     = 10,
        SQLITE_CORRUPT   = 11,
        SQLITE_NOTFOUND  = 12,
        SQLITE_FULL      = 13,
        SQLITE_CANTOPEN  = 14,
        SQLITE_PROTOCOL  = 15,
        SQLITE_EMPTY     = 16,
        SQLITE_SCHEMA    = 17,
        SQLITE_TOOBIG    = 18,
        SQLITE_CONSTRAINT= 19,
        SQLITE_MISMATCH  = 20,
        SQLITE_MISUSE    = 21,
        SQLITE_NOLFS     = 22,
        SQLITE_AUTH      = 23,
        SQLITE_FORMAT    = 24,
        SQLITE_RANGE     = 25,
        SQLITE_NOTADB    = 26
    }
}

/// Error results
#[derive(Debug, PartialEq, Eq)]
pub struct SqliteError {
    /// kind of error, by code
    pub kind: SqliteErrorCode,
    /// static error description
    pub desc: &'static str,
    /// dynamic detail (optional)
    pub detail: Option<String>
}

impl Display for SqliteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.detail {
            Some(ref x) => f.write_fmt(format_args!("{} ({})", x, self.kind as u32)),
            None => f.write_fmt(format_args!("{} ({})", self.desc, self.kind as u32))
        }
    }
}

impl SqliteError {
    /// Get a detailed description of the error
    pub fn detail(&self) -> Option<String> { self.detail.clone() }
}

impl Error for SqliteError {
    fn description(&self) -> &str { self.desc }
    fn cause(&self) -> Option<&dyn Error> { None }
}


/// Fundamental Datatypes
enum_from_primitive! {
    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    #[allow(non_camel_case_types)]
    #[allow(missing_docs)]
    pub enum ColumnType {
        SQLITE_INTEGER = 1,
        SQLITE_FLOAT   = 2,
        SQLITE_TEXT    = 3,
        SQLITE_BLOB    = 4,
        SQLITE_NULL    = 5
    }
}

#[cfg(test)]
mod bind_tests {
    use super::{DatabaseConnection, ResultSet};
    use super::{ResultRowAccess};
    use super::{SqliteResult};

    #[test]
    fn bind_fun() {
        fn go() -> SqliteResult<()> {
            let mut database = DatabaseConnection::in_memory()?;

            database.exec(
                "BEGIN;
                CREATE TABLE test (id int, name text, address text);
                INSERT INTO test (id, name, address) VALUES (1, 'John Doe', '123 w Pine');
                COMMIT;")?;

            {
                let mut tx = database.prepare(
                    "INSERT INTO test (id, name, address) VALUES (?, ?, ?)")?;
                assert_eq!(tx.bind_parameter_count(), 3);
                tx.bind_int(1, 2)?;
                tx.bind_text(2, "Jane Doe")?;
                tx.bind_text(3, "345 e Walnut")?;
                let mut results = tx.execute();
                assert!(results.step().ok().unwrap().is_none());
            }
            assert_eq!(database.changes(), 1);

            let mut q = database.prepare("select * from test order by id")?;
            let mut rows = q.execute();
            match rows.step() {
                Ok(Some(ref mut row)) => {
                    assert_eq!(row.get::<u32, i32>(0), 1);
                    // TODO let name = q.get_text(1);
                    // assert_eq!(name.as_slice(), "John Doe");
                },
                _ => panic!()
            }

            match rows.step() {
                Ok(Some(ref mut row)) => {
                    assert_eq!(row.get::<u32, i32>(0), 2);
                    //TODO let addr = q.get_text(2);
                    // assert_eq!(addr.as_slice(), "345 e Walnut");
                },
                _ => panic!()
            }
            Ok(())
        }
        match go() {
            Ok(_) => (),
            Err(e) => panic!("oops! {:?}", e)
        }
    }

    fn with_query<T, F>(sql: &str, mut f: F) -> SqliteResult<T>
        where F: FnMut(&mut ResultSet) -> T
    {
        let db = DatabaseConnection::in_memory()?;
        let mut s = db.prepare(sql)?;
        let mut rows = s.execute();
        let x = f(&mut rows);
        return Ok(x);
    }

    #[test]
    fn named_rowindex() {
        fn go() -> SqliteResult<(u32, i32)> {
            let mut count = 0;
            let mut sum = 0i32;

            with_query("select 1 as col1
                       union all
                       select 2", |rows| {
                loop {
                    match rows.step() {
                        Ok(Some(ref mut row)) => {
                            count += 1;
                            sum += row.column_int(0);
                        },
                        _ => break
                    }
                }
                (count, sum)
            })
        }
        assert_eq!(go(), Ok((2, 3)))
    }

    #[test]
    fn err_with_detail() {
        let io = || {
            let mut conn = DatabaseConnection::in_memory()?;
            conn.exec("CREATE gobbledygook")
        };

        let go = || match io() {
            Ok(_) => panic!(),
            Err(oops) => {
                format!("{:?}: {}: {}",
                        oops.kind, oops.desc,
                        oops.detail.unwrap())
            }
        };

        let expected = "SQLITE_ERROR: sqlite3_exec: near \"gobbledygook\": syntax error";
        assert_eq!(go(), expected.to_string())
    }
}

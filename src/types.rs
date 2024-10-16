//! Type conversions for binding parameters and getting query results.

use super::{PreparedStatement, ResultRow,
            ColIx, ParamIx};
use super::{
    SqliteResult,
};
use super::ColumnType::SQLITE_NULL;

/// Values that can be bound to parameters in prepared statements.
pub trait ToSql {
    /// Bind the `ix`th parameter to this value (`self`).
    fn to_sql(&self, s: &mut PreparedStatement, ix: ParamIx) -> SqliteResult<()>;
}

/// A trait for result values from a query.
///
/// cf [sqlite3 result values][column].
///
/// *inspired by sfackler's FromSql (and some haskell bindings?)*
///
/// [column]: http://www.sqlite.org/c3ref/column_blob.html
///
///   - *TODO: consider a `types` submodule*
///   - *TODO: many more implementors, including Option<T>*
pub trait FromSql<'a>: Sized {
    /// Try to extract a `Self` type value from the `col`th colum of a `ResultRow`.
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<Self>;
}

impl ToSql for i32 {
    fn to_sql(&self, s: &mut PreparedStatement, ix: ParamIx) -> SqliteResult<()> {
        s.bind_int(ix, *self)
    }
}

impl<'a> FromSql<'a> for i32 {
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<i32> { Ok(row.column_int(col)) }
}

impl ToSql for i64 {
    fn to_sql(&self, s: &mut PreparedStatement, ix: ParamIx) -> SqliteResult<()> {
        s.bind_int64(ix, *self)
    }
}

impl<'a> FromSql<'a> for i64 {
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<i64> { Ok(row.column_int64(col)) }
}

impl ToSql for f64 {
    fn to_sql(&self, s: &mut PreparedStatement, ix: ParamIx) -> SqliteResult<()> {
        s.bind_double(ix, *self)
    }
}

impl<'a> FromSql<'a> for f64 {
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<f64> { Ok(row.column_double(col)) }
}

impl ToSql for bool {
    fn to_sql(&self, s: &mut PreparedStatement, ix: ParamIx) -> SqliteResult<()> {
        s.bind_int(ix, if *self { 1 } else { 0 })
    }
}

impl<'a> FromSql<'a> for bool {
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<bool> { Ok(row.column_int(col)!=0) }
}

impl<T: ToSql + Clone> ToSql for Option<T> {
    fn to_sql(&self, s: &mut PreparedStatement, ix: ParamIx) -> SqliteResult<()> {
        match (*self).clone() {
            Some(x) => x.to_sql(s, ix),
            None => s.bind_null(ix)
        }
    }
}

impl<'a, T: FromSql<'a> + Clone> FromSql<'a> for Option<T> {
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<Option<T>> {
        match row.column_type(col) {
            SQLITE_NULL => Ok(None),
            _ => FromSql::from_sql(row, col).map(|x| Some(x))
        }
    }
}

impl ToSql for String {
    fn to_sql(&self, s: &mut PreparedStatement, ix: ParamIx) -> SqliteResult<()> {
        s.bind_text(ix, (*self).as_ref())
    }
}


impl<'a> FromSql<'a> for String {
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<String> {
        Ok(row.column_text(col).unwrap_or(String::new()))
    }
}

impl<'a> FromSql<'a> for &'a str {
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<&'a str> {
        Ok(row.column_str(col).unwrap_or(""))
    }
}

impl<'a> ToSql for &'a [u8] {
    fn to_sql(&self, s: &mut PreparedStatement, ix: ParamIx) -> SqliteResult<()> {
        s.bind_blob(ix, *self)
    }
}

impl<'a> FromSql<'a> for Vec<u8> {
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<Vec<u8>> {
        Ok(row.column_blob(col).unwrap_or(Vec::new()))
    }
}

impl<'a> FromSql<'a> for &'a [u8] {
    fn from_sql(row: &'a ResultRow, col: ColIx) -> SqliteResult<&'a [u8]> {
        Ok(row.column_slice(col).unwrap_or(&[]))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{DatabaseConnection, SqliteResult, ResultSet};
    use super::super::{ResultRowAccess};

    fn with_query<T, F>(sql: &str, mut f: F) -> SqliteResult<T>
        where F: FnMut(&mut ResultSet) -> T
    {
        let db = try!(DatabaseConnection::in_memory());
        let mut s = try!(db.prepare(sql));
        let mut rows = s.execute();
        Ok(f(&mut rows))
    }

    #[test]
    fn select_blob() {
        with_query("select x'ff0db0'", |results| {
            match results.step() {
                Ok(Some(ref mut row)) => {
                    let x : SqliteResult<Vec<u8>> = row.get_opt(0);
                    let x_slice: SqliteResult<&[u8]> = row.get_opt(0);
                    assert_eq!(x.ok().unwrap(), [0xff, 0x0d, 0xb0].to_vec());
                    assert_eq!(x_slice.ok().unwrap(), &[0xff, 0x0d, 0xb0]);
                },
                Ok(None) => panic!("no row"),
                Err(oops) =>  panic!("error: {:?}", oops)
            };
        }).unwrap();
    }
}

// Local Variables:
// flycheck-rust-crate-root: "lib.rs"
// End:

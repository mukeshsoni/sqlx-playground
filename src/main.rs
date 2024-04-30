use sqlx::{Row, SqlitePool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::Row;
    use sqlx::SqlitePool;

    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc"))]
    async fn async_test(pool: SqlitePool) -> sqlx::Result<()> {
        let foo = sqlx::query("SELECT * FROM library_file")
            .fetch_all(&pool)
            .await?;

        assert_eq!(foo.len(), 20);
        let foo = sqlx::query("SELECT * FROM library_file where base_name='abc'")
            .fetch_one(&pool)
            .await?;
        assert_eq!(foo.get::<String, _>("base_name"), "abc");
        // assert!(false);
        Ok(())
    }

    #[test]
    fn it_works() {
        assert_eq!(2, 2);
    }
}

use crate::image_helpers;
use chrono::prelude::{DateTime, Utc};
use sqlx::{Execute, QueryBuilder, Row, Sqlite, SqlitePool, Transaction};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    pub rating: u32,
    pub flag: String,
    pub color_label: String,
}

#[derive(sqlx::FromRow, serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Image {
    original_file_name: String,
    extension: String,
    file_created_time: String,
    path: String,
    parent_path: String,
    rating: u32,
    flag: String,
    color_label: String,
    capture_time: String,
    file_width: u32,
    file_height: u32,
}

pub async fn has_images_for_path(
    pool: &SqlitePool,
    folder_path: &str,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("select count(*) as count from library_file where parent_path=$1")
        .bind(folder_path)
        .fetch_one(pool)
        .await?;

    let has_images = row.get::<u32, _>("count") > 0;
    Ok(has_images)
}

async fn get_images_in_path(
    pool: &SqlitePool,
    path: &str,
    sort_option: &str,
    sort_order: &str,
    filter: &Filter,
) -> Result<Vec<Image>, sqlx::Error> {
    // We use json_group_array so that instead of getting multiple rows for each image tag,
    // we group all the tags for particular image into an array
    let mut query_builder = QueryBuilder::new(
        r#"select *, json_group_array(t.tag_name) as tags from library_file as lf join image as i on lf.id == i.library_file_id left join tag as t on t.image_id = i.id
        where lf.parent_path=? and i.rating >= ?"#,
    );
    if filter.flag != "unpicked" {
        query_builder.push(" and i.flag=?");
    }
    if filter.color_label != "none" {
        query_builder.push(" and i.color_label=?");
    }
    query_builder.push(" group by i.id");

    if sort_option != "default" {
        query_builder.push(" order by ".to_string() + sort_option + " " + sort_order);
    }
    let mut query = query_builder.build_query_as::<Image>();
    query = query.bind(path).bind(filter.rating);
    if filter.flag != "unpicked" {
        query = query.bind(&filter.flag);
    }
    if filter.color_label != "none" {
        query = query.bind(&filter.color_label);
    }

    println!("Query: {:?}", query.sql());
    let query_result = query.fetch_all(pool).await?;
    println!("Number of images: {}", query_result.len());
    Ok(query_result)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirContent {
    pub name: String,
    pub path: PathBuf,
    pub parent_path: String,
    pub extension: String,
    pub is_directory: bool,
}

#[derive(sqlx::FromRow, serde::Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct LibraryFile {
    original_file_name: String,
    base_name: String,
    extension: String,
    file_created_time: String,
    file_modified_time: String,
    path: String,
    parent_path: String,
}

fn path_to_string(path: &Path) -> String {
    path.to_owned().to_string_lossy().to_owned().to_string()
}

/// function which returns the contents of a given directory
/// The argument is a direct path to the directory
fn get_dir_image_files(dir_path: &str) -> Result<Vec<LibraryFile>, std::io::Error> {
    let entries = fs::read_dir(dir_path).expect("Could not read directory content");
    let entries = entries
        // Didn't know about filter_map being used to get value out of Option's inside
        // an iterator
        .filter_map(|entry| entry.ok())
        .filter(|entry| !entry.metadata().unwrap().is_dir())
        .filter(|entry| image_helpers::is_image_file(&entry.path()));
    let mut contents = vec![];

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_str().unwrap().to_string();
        // Is it better to keep parent_path as Path?
        let parent_path = path.parent().unwrap();
        let metadata = fs::metadata(&path)?;
        let created_time: DateTime<Utc> = metadata.created().unwrap().into();
        let modified_time: DateTime<Utc> = metadata.modified().unwrap().into();
        // How to convert rust SystemTime into ISO8601 string using chrono
        // https://stackoverflow.com/a/64148017
        // I have actually used the solution given by first comment in the answer
        // So that i have something similar to what javascript toISOString() method returns
        let file_created_time = created_time.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let file_modified_time = modified_time.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        // metadata.
        contents.push(LibraryFile {
            path: path_to_string(&path),
            parent_path: path_to_string(parent_path),
            original_file_name: name,
            base_name: path
                .file_stem()
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap(),
            extension: path
                .extension()
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap(),
            file_created_time,
            file_modified_time,
        });
    }
    Ok(contents)
}

async fn insert_library_file<'a>(
    mut tx: &mut Transaction<'a, Sqlite>,
    file: &LibraryFile,
) -> Result<i64, sqlx::Error> {
    let query = sqlx::query("INSERT INTO library_file (original_file_name, base_name, extension, file_created_time, file_modified_time, path, parent_path) values (?, ?, ?, ?, ?, ?, ?)")
        .bind(&file.original_file_name)
        .bind(&file.base_name)
        .bind(&file.extension)
        .bind(&file.file_created_time)
        .bind(&file.file_modified_time)
        .bind(&file.path)
        .bind(&file.parent_path)
        .execute(&mut **tx)
        .await?;
    Ok(query.last_insert_rowid())
}

// It takes a library_file_id which is sent after library_file row is inserted
// And it takes image path so that it can extract exif information from it. It mainly
// needs the image creation date so that we can sort by image creation date.
async fn insert_image_details<'a>(
    mut tx: &mut Transaction<'a, Sqlite>,
    library_file_id: i64,
    library_file: &LibraryFile,
) -> Result<i64, sqlx::Error> {
    let meta = rexiv2::Metadata::new_from_path(Path::new(&library_file.path));
    let mut image_capture_time: String = library_file.file_created_time.clone();

    if meta.is_ok() {
        let meta = meta.unwrap();
        if meta.has_tag("Exif.Image.DateTime") {
            image_capture_time = meta.get_tag_string("Exif.Image.DateTime").unwrap();
        }
    }
    let query = sqlx::query("INSERT INTO image (library_file_id, capture_time) values (?, ?)")
        .bind(library_file_id)
        .bind(image_capture_time)
        .execute(&mut **tx)
        .await?;

    Ok(query.last_insert_rowid())
}

// Find all images in the given path, extract exif and other metadata for the images and
// insert into the database
pub async fn insert_images(pool: &SqlitePool, path: &str) -> Result<(), sqlx::Error> {
    // We will run all insert queries inside a transaction so that inserts are fast
    match get_dir_image_files(path) {
        Ok(dir_image_files) => {
            let mut conn = pool.begin().await?;
            // todo
            // First insert library_file
            // Then insert image
            // Then insert exif
            // Then insert iptc
            for file in dir_image_files {
                // println!("{file:?}");
                let library_file_id = insert_library_file(&mut conn, &file).await?;
                insert_image_details(&mut conn, library_file_id, &file).await?;
            }
            conn.commit().await?;
            Ok(())
        }
        Err(err) => return Ok(()),
    }
}

pub async fn get_keywords(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query("SELECT DISTINCT tag_name from tag")
        .fetch_all(pool)
        .await?;

    let mut keywords = vec![];
    for row in rows {
        keywords.push(row.get::<String, _>("tag_name"));
    }
    Ok(keywords)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use sqlx::Row;
    use sqlx::SqlitePool;

    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc"))]
    async fn test_has_images_for_path(pool: SqlitePool) -> sqlx::Result<()> {
        let has_images = has_images_for_path(&pool, "/Users/fancy-name/Desktop").await?;
        assert_eq!(has_images, true);
        let has_images = has_images_for_path(&pool, "/Users/fancy-name/nonexistentpath").await?;
        assert_eq!(has_images, false);
        Ok(())
    }

    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc"))]
    async fn test_get_images(pool: SqlitePool) -> sqlx::Result<()> {
        let filter = Filter {
            rating: 0,
            flag: "unpicked".to_string(),
            color_label: "none".to_string(),
        };
        let images = get_images_in_path(
            &pool,
            "/Users/fancy-name/Desktop",
            "default",
            "asc",
            &filter,
        )
        .await?;
        assert_eq!(images.len(), 16);
        // Get images with rating 2 or above
        let filter = Filter {
            rating: 2,
            flag: "unpicked".to_string(),
            color_label: "none".to_string(),
        };
        let images = get_images_in_path(
            &pool,
            "/Users/fancy-name/Desktop",
            "default",
            "asc",
            &filter,
        )
        .await?;
        assert_eq!(images.len(), 12);

        // Get images with rating 3 or above and color_label "green"
        let filter = Filter {
            rating: 3,
            flag: "unpicked".to_string(),
            color_label: "green".to_string(),
        };
        let images = get_images_in_path(
            &pool,
            "/Users/fancy-name/Desktop",
            "default",
            "asc",
            &filter,
        )
        .await?;
        assert_eq!(images.len(), 3);
        // Get images with rating 3 or above and color_label "green" and flag as "picked"
        let filter = Filter {
            rating: 3,
            flag: "picked".to_string(),
            color_label: "green".to_string(),
        };
        let images = get_images_in_path(
            &pool,
            "/Users/fancy-name/Desktop",
            "default",
            "asc",
            &filter,
        )
        .await?;
        assert_eq!(images.len(), 2);
        Ok(())
    }

    // Testing insert_images will need file system access
    // We need to have some dummy images in some folder inside our project maybe
    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc"))]
    async fn test_insert_images(pool: SqlitePool) -> sqlx::Result<()> {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = manifest_dir.to_string() + "/test_image_files";
        let filter = Filter {
            rating: 0,
            flag: "unpicked".to_string(),
            color_label: "none".to_string(),
        };
        let images = get_images_in_path(&pool, &path, "default", "asc", &filter).await?;
        assert_eq!(images.len(), 0);

        let now = Instant::now();
        insert_images(&pool, &path).await?;
        println!("Time elapsed for bulk insertion {:?}", now.elapsed());

        let dirs = fs::read_dir(&path)?;
        let images = get_images_in_path(&pool, &path, "default", "asc", &filter).await?;
        assert_eq!(images.len(), dirs.count());

        Ok(())
    }

    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc", "tag"))]
    async fn test_get_keywords(pool: SqlitePool) -> sqlx::Result<()> {
        let keywords = get_keywords(&pool).await?;
        assert_eq!(keywords.len(), 11);
        Ok(())
    }
}

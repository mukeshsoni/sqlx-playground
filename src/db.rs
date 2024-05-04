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

async fn insert_exif_data<'a>(
    tx: &mut Transaction<'a, Sqlite>,
    image_id: i64,
    meta: &rexiv2::Metadata,
) -> Result<i64, sqlx::Error> {
    // TODO: Move this vector to a global constant
    let exif_columns = vec![
        ("camera_make", "Exif.Image.Make"),
        ("camera_model", "Exif.Image.Model"),
        ("exposure_time", "Exif.Image.ExposureTime"),
        ("f_number", "Exif.Image.FNumber"),
        ("exposure_program", "Exif.Image.ExposureProgram"),
        ("iso_speed", "Exif.Photo.ISOSpeed"),
        ("exif_version", "Exif.Photo.ExifVersion"),
        ("datetime_original", "Exif.Photo.DateTimeOriginal"),
        ("offset_time_original", "Exif.Photo.OffsetTimeOriginal"),
        ("shutter_speed", "Exif.Photo.ShutterSpeedValue"),
        ("aperture_value", "Exif.Photo.ApertureValue"),
        ("brightness_value", "Exif.Photo.BrightnessValue"),
        ("metering_mode", "Exif.Photo.MeteringMode"),
        ("flash", "Exif.Photo.Flash"),
        ("exposure_mode", "Exif.Photo.ExposureMode"),
        ("white_balance", "Exif.Photo.WhiteBalance"),
        ("focal_length", "Exif.Photo.FocalLength"),
        (
            "focal_length_in_35mm_film",
            "Exif.Photo.FocalLengthIn35mmFilm",
        ),
        ("sharpness", "Exif.Photo.Sharpness"),
        ("lens_specification", "Exif.Photo.LensSpecification"),
        ("lens_make", "Exif.Photo.LensMake"),
        ("lens_model", "Exif.Photo.LensModel"),
        ("body_serial_number", "Exif.Photo.BodySerialNumber"),
        ("saturation", "Exif.Photo.Saturation"),
        ("contrast", "Exif.Photo.Contrast"),
        ("gps_latitude", "Exif.GPSInfo.GPSLatitude"),
        ("gps_longitude", "Exif.GPSInfo.GPSLongitude"),
        ("gps_altitude", "Exif.GPSInfo.GPSAltitude"),
        ("gps_timestamp", "Exif.GPSInfo.GPSTimeStamp"),
        ("gps_status", "Exif.GPSInfo.GPSStatus"),
        ("artist", "Exif.Image.Artist"),
    ];
    // Figuring the query builder part took me 2 days!
    let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new("INSERT INTO exif (");
    for exif_column in exif_columns.clone() {
        if let Ok(_column_value) = meta.get_tag_string(exif_column.1) {
            query_builder.push(exif_column.0.to_string() + ", ");
        }
    }
    query_builder.push("image_id ");
    query_builder.push(" ) VALUES (");
    let mut separated = query_builder.separated(", ");
    // Why are we running the loop twice?
    // Because we are building the column names list in the sql, e.g. (camera_make, lens_model) etc., in the first pass
    // And building the (?) list and binding values to them in the second pass
    // because query_builder is mutably borrowed by both push and separated.push_bind methods
    // And we can only borrow it mutably once
    for exif_column in exif_columns {
        if let Ok(column_value) = meta.get_tag_string(exif_column.1) {
            separated.push_bind(column_value);
        }
    }
    separated.push_bind(image_id);
    separated.push_unseparated(" )");
    let query = query_builder.build();
    let query_result = query.execute(&mut **tx).await?;
    Ok(query_result.last_insert_rowid())
}

async fn insert_iptc_data<'a>(
    tx: &mut Transaction<'a, Sqlite>,
    image_id: i64,
    meta: &rexiv2::Metadata,
) -> Result<i64, sqlx::Error> {
    // TODO: Move this vector to a global constant
    let iptc_columns = vec![
        ("copyright", "Iptc.Application2.Copyright"),
        ("city", "Iptc.Application2.City"),
        ("creator", "Iptc.Application2.Byline"),
        ("country_iso_code", "Iptc.Application2.CountryCode"),
        ("country_name", "Iptc.Application2.CountryName"),
        ("description", "Iptc.Application2.Caption"),
    ];
    // Figuring the query builder part took me 2 days!
    let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new("INSERT INTO iptc (");
    for iptc_column in iptc_columns.clone() {
        if let Ok(_column_value) = meta.get_tag_string(iptc_column.1) {
            query_builder.push(iptc_column.0.to_string() + ", ");
        }
    }
    query_builder.push("image_id ");
    query_builder.push(" ) VALUES (");
    let mut separated = query_builder.separated(", ");
    // Why are we running the loop twice?
    // Because we are building the column names list in the sql, e.g. (camera_make, lens_model) etc., in the first pass
    // And building the (?) list and binding values to them in the second pass
    // because query_builder is mutably borrowed by both push and separated.push_bind methods
    // And we can only borrow it mutably once
    for iptc_column in iptc_columns {
        if let Ok(column_value) = meta.get_tag_string(iptc_column.1) {
            separated.push_bind(column_value);
        }
    }
    separated.push_bind(image_id);
    separated.push_unseparated(" )");
    let query = query_builder.build();
    let query_result = query.execute(&mut **tx).await?;
    Ok(query_result.last_insert_rowid())
}

async fn write_exif_and_iptc_to_db<'a>(
    tx: &mut Transaction<'a, Sqlite>,
    library_file: &LibraryFile,
    image_id: i64,
) -> Result<(i64, i64), sqlx::Error> {
    let image_path = &library_file.path;

    let meta = rexiv2::Metadata::new_from_path(Path::new(image_path));

    match meta {
        Ok(meta) => {
            let exif_id = insert_exif_data(tx, image_id, &meta).await?;
            let iptc_id = insert_iptc_data(tx, image_id, &meta).await?;
            Ok((exif_id, iptc_id))
        }
        Err(_e) => {
            // When we can't read exif, we insert the file creation date as datetime_original
            // If we had exif, datetime_original would correspond to the date and time the
            // photo was taken
            let file_creation_time = library_file.file_created_time.clone();
            let exif_query_result =
                sqlx::query("Insert into exif (image_id, , datetime_original) values (?, ?, ?)")
                    .bind(image_id)
                    .bind(file_creation_time.to_string())
                    .execute(&mut **tx)
                    .await?;
            let iptc_query_result = sqlx::query("Insert into iptc (image_id) values (?, ?, ?)")
                .bind(image_id)
                .execute(&mut **tx)
                .await?;
            Ok((
                exif_query_result.last_insert_rowid(),
                iptc_query_result.last_insert_rowid(),
            ))
        }
    }
}

async fn insert_library_file<'a>(
    tx: &mut Transaction<'a, Sqlite>,
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
    tx: &mut Transaction<'a, Sqlite>,
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
                let image_id = insert_image_details(&mut conn, library_file_id, &file).await?;
                write_exif_and_iptc_to_db(&mut conn, &file, image_id).await?;
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

async fn get_image_id_from_path(pool: &SqlitePool, image_path: &str) -> Result<u32, sqlx::Error> {
    let row = sqlx::query("Select image.id from image left join library_file on image.library_file_id=library_file.id where library_file.path=$1")
        .bind(image_path)
        .fetch_one(pool)
        .await?;

    Ok(row.get::<u32, _>("id"))
}

pub async fn add_keyword(
    pool: &SqlitePool,
    image_path: &str,
    keyword: &str,
) -> Result<(), sqlx::Error> {
    let image_id = get_image_id_from_path(pool, image_path).await?;

    sqlx::query("INSERT into tag (image_id, tag_name) values (?, ?)")
        .bind(image_id)
        .bind(&keyword)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn remove_keyword(
    pool: &SqlitePool,
    image_path: &str,
    keyword: &str,
) -> Result<(), sqlx::Error> {
    let image_id = get_image_id_from_path(pool, image_path).await?;

    sqlx::query("DELETE from tag where image_id=$1 and tag_name=$2")
        .bind(image_id)
        .bind(&keyword)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_image_rating(
    pool: &SqlitePool,
    image_path: &str,
    rating: u32,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE image set rating=? from library_file lf where lf.id=image.library_file_id and lf.path=?")
        .bind(rating)
        .bind(image_path)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_color_label(
    pool: &SqlitePool,
    image_path: &str,
    color_label: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE image set color_label=? from library_file lf where lf.id=image.library_file_id and lf.path=?")
        .bind(&color_label)
        .bind(&image_path)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_flag(
    pool: &SqlitePool,
    image_path: &str,
    flag: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE image set flag=? from library_file lf where lf.id=image.library_file_id and lf.path=?")
        .bind(flag)
        .bind(image_path)
        .execute(pool)
        .await?;

    Ok(())
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

    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc", "tag"))]
    async fn test_add_keyword(pool: SqlitePool) -> sqlx::Result<()> {
        let row = sqlx::query("Select count(*) as count from tag")
            .fetch_one(&pool)
            .await?;
        let keyword_count = row.get::<u32, _>("count");
        assert_eq!(keyword_count, 20);

        add_keyword(&pool, "/Users/fancy-name/Desktop/abc.jpg", "random_stuff").await?;

        let row = sqlx::query("Select count(*) as count from tag")
            .fetch_one(&pool)
            .await?;
        let keyword_count = row.get::<u32, _>("count");
        assert_eq!(keyword_count, 21);

        Ok(())
    }

    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc", "tag"))]
    async fn test_remove_keyword(pool: SqlitePool) -> sqlx::Result<()> {
        let row = sqlx::query("Select count(*) as count from tag")
            .fetch_one(&pool)
            .await?;
        let keyword_count = row.get::<u32, _>("count");
        assert_eq!(keyword_count, 20);

        remove_keyword(&pool, "/Users/fancy-name/Desktop/abc.jpg", "nature").await?;

        let row = sqlx::query("Select count(*) as count from tag")
            .fetch_one(&pool)
            .await?;
        let keyword_count = row.get::<u32, _>("count");
        assert_eq!(keyword_count, 19);
        Ok(())
    }

    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc", "tag"))]
    async fn test_update_flag(pool: SqlitePool) -> sqlx::Result<()> {
        let image_path = "/Users/fancy-name/Desktop/abc.jpg";
        let flag = "rejected";
        update_flag(&pool, image_path, flag).await?;
        let query_result = sqlx::query("Select flag from image left join library_file on library_file.id=image.library_file_id where library_file.path=?")
            .bind(image_path)
            .fetch_one(&pool)
            .await?;
        let flag = query_result.get::<String, _>("flag");
        assert_eq!(flag, "rejected");
        Ok(())
    }

    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc", "tag"))]
    async fn test_update_image_rating(pool: SqlitePool) -> sqlx::Result<()> {
        let image_path = "/Users/fancy-name/Desktop/abc.jpg";
        let rating = 0;
        update_image_rating(&pool, image_path, rating).await?;
        let query_result = sqlx::query("Select rating from image left join library_file on library_file.id=image.library_file_id where library_file.path=?")
            .bind(image_path)
            .fetch_one(&pool)
            .await?;
        let rating = query_result.get::<u32, _>("rating");
        assert_eq!(rating, 0);
        Ok(())
    }

    #[sqlx::test(fixtures("library_file", "image", "exif", "iptc", "tag"))]
    async fn test_update_color_label(pool: SqlitePool) -> sqlx::Result<()> {
        let image_path = "/Users/fancy-name/Desktop/abc.jpg";
        let color_label = "blue";
        update_color_label(&pool, image_path, color_label).await?;
        let query_result = sqlx::query("Select color_label from image left join library_file on library_file.id=image.library_file_id where library_file.path=?")
            .bind(image_path)
            .fetch_one(&pool)
            .await?;
        let color_label = query_result.get::<String, _>("color_label");
        assert_eq!(color_label, "blue");
        Ok(())
    }
}

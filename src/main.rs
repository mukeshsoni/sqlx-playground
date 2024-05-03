use sqlx::{QueryBuilder, Row, SqlitePool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");
    Ok(())
}

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
    // let query = sqlx::query_as::<_, image_helpers::Image>(query.sql())
    //     .bind(path)
    //     .bind(filter.rating);
    let query_result = query.fetch_all(pool).await?;
    println!("Number of images: {}", query_result.len());
    Ok(query_result)
}

#[cfg(test)]
mod tests {
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
        let images =
            get_images_in_path(&pool, "/Users/fancy-name/Desktop", "default", "asc", &filter).await?;
        println!("Images {images:#?}");
        assert_eq!(images.len(), 16);
        // Get images with rating 2 or above
        let filter = Filter {
            rating: 2,
            flag: "unpicked".to_string(),
            color_label: "none".to_string(),
        };
        let images =
            get_images_in_path(&pool, "/Users/fancy-name/Desktop", "default", "asc", &filter).await?;
        println!("Images {images:#?}");
        assert_eq!(images.len(), 12);

        // Get images with rating 3 or above and color_label "green"
        let filter = Filter {
            rating: 3,
            flag: "unpicked".to_string(),
            color_label: "green".to_string(),
        };
        let images =
            get_images_in_path(&pool, "/Users/fancy-name/Desktop", "default", "asc", &filter).await?;
        println!("Images {images:#?}");
        assert_eq!(images.len(), 3);
        // Get images with rating 3 or above and color_label "green" and flag as "picked"
        let filter = Filter {
            rating: 3,
            flag: "picked".to_string(),
            color_label: "green".to_string(),
        };
        let images =
            get_images_in_path(&pool, "/Users/fancy-name/Desktop", "default", "asc", &filter).await?;
        println!("Images {images:#?}");
        assert_eq!(images.len(), 2);
        Ok(())
    }

    #[test]
    fn it_works() {
        assert_eq!(2, 2);
    }
}

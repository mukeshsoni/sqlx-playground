use std::path::Path;

// TODO: What if instead we represented exif as vector of tuples like this Vec<(exif::Tag, ValueTypeEnum)>?
// That will make it easier to construct from the exif.fields() from exif crate
// And it will still be possible to iterate over the vector and construct the INSERT sql
// query. We will have to make sure that the database columns correspond exactly to
// string representations of exif::Tag enums
#[derive(sqlx::FromRow, serde::Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Exif {
    pub image_id: i64,
    pub created_at: String,
    pub modified_at: String,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub x_resolution: Option<String>,
    pub y_resolution: Option<String>,
    pub resolution_unit: Option<String>,
    pub software: Option<String>,
    pub copyright: Option<String>,
    pub exposure_time: Option<String>,
    pub f_number: Option<String>,
    pub exposure_program: Option<String>,
    pub photographic_sensitivty: Option<String>,
    pub sensitivity_type: Option<String>,
    pub iso_speed: Option<String>,
    pub exif_version: Option<String>,
    pub datetime: Option<String>,
    pub offset_time: Option<String>,
    pub datetime_original: Option<String>,
    pub offset_time_original: Option<String>,
    pub shutter_speed_value: Option<String>,
    pub aperture_value: Option<String>,
    pub brightness_value: Option<String>,
    pub max_aperture_value: Option<String>,
    pub metering_mode: Option<String>,
    pub light_source: Option<String>,
    pub flash: Option<String>,
    pub focal_length: Option<String>,
    pub pixel_x_dimension: Option<String>,
    pub pixel_y_dimension: Option<String>,
    pub sensing_method: Option<String>,
    pub file_source: Option<String>,
    pub scene_type: Option<String>,
    pub custom_rendered: Option<String>,
    pub exposure_mode: Option<String>,
    pub white_balance: Option<String>,
    pub focal_length_in_35mm_film: Option<String>,
    pub scene_capture_type: Option<String>,
    pub sharpness: Option<String>,
    pub subject_distance: Option<String>,
    pub subject_distance_range: Option<String>,
    pub lens_specification: Option<String>,
    pub lens_make: Option<String>,
    pub lens_model: Option<String>,
    pub compression: Option<String>,
    pub body_serial_number: Option<String>,
    pub saturation: Option<String>,
    pub contrast: Option<String>,
    pub gps_latitude: Option<String>,
    pub gps_longitude: Option<String>,
    pub gps_altitude: Option<String>,
    pub gps_timestamp: Option<String>,
    pub gps_status: Option<String>,
    pub orientation: Option<String>,
    // pub title: Option<String>,
    pub description: Option<String>,
    pub user_comment: Option<String>,
    pub artist: Option<String>,
}

pub fn is_image_file(path: &Path) -> bool {
    is_regular_image(path) || is_raw_image(path)
}
pub fn is_regular_image(path: &Path) -> bool {
    let image_extensions = [
        "jpg", "png", "tif", "jpeg", "jpe", "gif", "bmp", "webp", "tiff",
    ];
    if let Some(extension) = path.extension() {
        let extension = extension.to_ascii_lowercase();
        let extension = extension.to_string_lossy();
        image_extensions.contains(&extension.as_ref())
    } else {
        return false;
    }
}
pub fn is_raw_image(path: &Path) -> bool {
    let raw_image_extensions = [
        "raf", "cr2", "mrw", "arw", "srf", "sr2", "mef", "orf", "srw", "erf", "kdc", "dcs", "rw2",
        "dcr", "dng", "pef", "crw", "raw", "iiq", "3rf", "nrw", "nef", "mos", "ari",
    ];
    if let Some(extension) = path.extension() {
        let extension = extension.to_ascii_lowercase();
        let extension = extension.to_string_lossy();
        raw_image_extensions.contains(&extension.as_ref())
    } else {
        return false;
    }
}

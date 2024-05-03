create table if not exists library_file (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    original_file_name NOT NULL DEFAULT '',
    base_name NOT NULL DEFAULT '',
    extension NOT NULL DEFAULT '',
    file_created_time DATETIME,
    external_mod_time DATETIME,
    path NOT NULL DEFAULT '',
    parent_path NOT NULL DEFAULT '',
    created_at DATETIME not null DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP -- This should be modified using a trigger
);

create table if not exists image (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    library_file_id INTEGER,
    created_at DATETIME not null DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP, -- This should be modified using a trigger
    rating integer DEFAULT 0,
    flag varchar DEFAULT "unpicked", -- can be 'picked', 'rejected' or 'unpicked'
    color_label varchar DEFAULT "none" not null,
    aspect_ratio_cache NOT NULL DEFAULT -1,
    bit_depth NOT NULL DEFAULT 0,
    capture_time,
    color_channels NOT NULL DEFAULT 0,
    color_labels NOT NULL DEFAULT '',
    edit_lock INTEGER NOT NULL DEFAULT 0,
    file_format NOT NULL DEFAULT 'unset',
    file_height,
    file_width,
    orientation,
    original_capture_time,
    panning_distance_h,
    panning_distance_v,
    FOREIGN KEY(library_file_id) REFERENCES library_file(id)
);

create table if not exists exif (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    image_id INTEGER NOT NULL,
    created_at DATETIME not null DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP, -- This should be modified using a trigger
    camera_make varchar,
    camera_model varchar,
    exposure_time varchar,
    f_number varchar,
    exposure_program varchar,
    iso_speed varchar,
    exif_version varchar,
    datetime_original varchar,
    offset_time_original varchar,
    shutter_speed varchar,
    aperture_value varchar,
    brightness_value varchar,
    metering_mode varchar,
    flash varchar,
    exposure_mode varchar,
    white_balance varchar,
    focal_length varchar,
    focal_length_in_35mm_film varchar,
    sharpness varchar,
    lens_specification varchar,
    lens_make varchar,
    lens_model varchar,
    body_serial_number varchar,
    saturation varchar,
    contrast varchar,
    gps_latitude varchar,
    gps_longitude varchar,
    gps_altitude varchar,
    gps_timestamp varchar,
    gps_status varchar,
    artist varchar,
    FOREIGN KEY(image_id) REFERENCES image(id)
);

create table if not exists iptc (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    image_id INTEGER NOT NULL,
    copyright varchar,
    city varchar,
    country varchar,
    creator varchar,
    country_iso_code varchar,
    country_name varchar,
    description varchar,
    FOREIGN KEY(image_id) REFERENCES image(id)
);

create table if not exists tag (
    image_id INTEGER NOT NULL,
    tag_name varchar NOT NULL,
    FOREIGN KEY(image_id) REFERENCES image(id),
    UNIQUE (image_id, tag_name)
);

create table if not exists thumbnail (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    image_id INTEGER NOT NULL,
    height INTEGER not null,
    width INTEGER not null,
    imagedata blob not null,
    FOREIGN KEY(image_id) REFERENCES image(id)
);

create table if not exists tag (
    image_id INTEGER NOT NULL,
    tag_name varchar NOT NULL,
    FOREIGN KEY(image_id) REFERENCES image(id),
    UNIQUE (image_id, tag_name)
);

create unique index IF NOT EXISTS image_path on library_file(path);
create index IF NOT EXISTS image_parent_path on library_file(parent_path);

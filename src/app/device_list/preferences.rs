//! Sorting preferences live in the user's platform-specific configuration directory.

use super::sort::DeviceSort;
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

pub(super) struct SortStorage {
    path: PathBuf,
}

impl SortStorage {
    pub fn for_user() -> Result<Self, String> {
        let directory = dirs::config_dir().ok_or("无法获取用户配置目录")?;
        Ok(Self::new(
            directory.join("bandix-gui").join("device-sort.json"),
        ))
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<DeviceSort, String> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(DeviceSort::default());
            }
            Err(error) => return Err(error.to_string()),
        };
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    }

    pub fn save(&self, sort: DeviceSort) -> Result<(), String> {
        self.write(sort).map_err(|error| error.to_string())
    }

    fn write(&self, sort: DeviceSort) -> io::Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "配置路径缺少父目录"))?;
        fs::create_dir_all(parent)?;
        // A same-directory temporary file keeps replacement on the same filesystem.
        // tempfile::persist also replaces existing destinations on Windows.
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(temporary.as_file_mut(), &sort)?;
        temporary.write_all(b"\n")?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::device_list::sort::{SortDirection, SortField};

    #[test]
    fn missing_file_is_read_only_and_every_sort_choice_round_trips_across_reopens() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("配置目录").join("device-sort.json");
        assert_eq!(
            SortStorage::new(path.clone()).load().unwrap(),
            DeviceSort::default()
        );
        assert!(
            !path.parent().unwrap().exists(),
            "loading must not create files"
        );
        for field in SortField::ALL {
            for direction in [SortDirection::Ascending, SortDirection::Descending] {
                for online_first in [false, true] {
                    let sort = DeviceSort {
                        field,
                        direction,
                        online_first,
                    };
                    SortStorage::new(path.clone()).save(sort).unwrap();
                    assert_eq!(SortStorage::new(path.clone()).load().unwrap(), sort);
                }
            }
        }
        let storage = SortStorage::new(path.clone());
        storage.save(DeviceSort::default()).unwrap();
        assert_eq!(
            SortStorage::new(path.clone()).load().unwrap(),
            DeviceSort::default()
        );
        assert_eq!(
            fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1,
            "temporary files are cleaned up"
        );
    }

    #[test]
    fn partial_and_future_options_use_defaults_and_corrupt_files_can_be_replaced() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("device-sort.json");
        let storage = SortStorage::new(path.clone());
        fs::write(
            &path,
            br#"{"field":"wan_download","online_first":true,"future_option":42}"#,
        )
        .unwrap();
        assert_eq!(
            storage.load().unwrap(),
            DeviceSort {
                field: SortField::WanDownload,
                direction: SortDirection::Ascending,
                online_first: true,
            }
        );
        fs::write(
            &path,
            br#"{"field":"future_field","direction":"future_direction","online_first":true}"#,
        )
        .unwrap();
        assert_eq!(
            storage.load().unwrap(),
            DeviceSort {
                online_first: true,
                ..DeviceSort::default()
            }
        );
        fs::write(&path, b"{incomplete").unwrap();
        assert!(storage.load().is_err());
        storage.save(DeviceSort::default()).unwrap();
        assert_eq!(storage.load().unwrap(), DeviceSort::default());
    }

    #[test]
    fn failed_replacement_reports_error_without_removing_the_existing_destination() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("device-sort.json");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), b"existing data").unwrap();
        assert!(
            SortStorage::new(path.clone())
                .save(DeviceSort::default())
                .is_err()
        );
        assert_eq!(fs::read(path.join("keep")).unwrap(), b"existing data");
        assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 1);
    }
}

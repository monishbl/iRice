//! What the picker lists, as WallpaperPicker.qml's two folder models: the
//! wallpaper folder's pictures, newest first (*.jpg *.jpeg *.png *.webp *.JPG
//! *.PNG, no hidden files), then the bundled ones by name (*.jpg).

use std::path::PathBuf;

/// The user's folder's pictures, newest first, then the bundled ones by name
pub fn list(dir: &str, bundled: &[PathBuf]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut mine: Vec<(std::time::SystemTime, String)> = rd
            .flatten()
            .filter(|e| user_picture(&e.file_name().to_string_lossy()))
            .filter_map(|e| {
                let m = std::fs::metadata(e.path()).ok().filter(|m| m.is_file())?;
                Some((m.modified().unwrap_or(std::time::UNIX_EPOCH), e.path().to_string_lossy().into_owned()))
            })
            .collect();
        mine.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        out.extend(mine.into_iter().map(|(_, p)| p));
    }
    let mut ours: Vec<(String, String)> = Vec::new();
    for d in bundled {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".jpg") && !name.starts_with('.') && std::fs::metadata(e.path()).is_ok_and(|m| m.is_file()) {
                ours.push((name, e.path().to_string_lossy().into_owned()));
            }
        }
    }
    ours.sort();
    out.extend(ours.into_iter().map(|(_, p)| p));
    out
}

/// FolderListModel's name filters (they match case: a .JPEG or .WEBP isn't listed)
fn user_picture(name: &str) -> bool {
    !name.starts_with('.') && ["jpg", "jpeg", "png", "webp", "JPG", "PNG"].iter().any(|x| name.strip_suffix(x).is_some_and(|s| s.ends_with('.')))
}

/// Where a picture is in the list: by its path, else by where it really is
/// (a picture chosen from the Material look's root of links is the same file
/// as the bundled one)
pub fn index_of(files: &[String], picture: &str) -> Option<usize> {
    if picture.is_empty() {
        return None;
    }
    if let Some(i) = files.iter().position(|f| f == picture) {
        return Some(i);
    }
    let real = std::fs::canonicalize(picture).ok()?;
    files.iter().position(|f| std::fs::canonicalize(f).is_ok_and(|r| r == real))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_filters_as_the_qml() {
        for n in ["a.jpg", "b.jpeg", "c.png", "d.webp", "E.JPG", "F.PNG"] {
            assert!(user_picture(n), "{n}");
        }
        for n in ["g.JPEG", "h.WEBP", "i.gif", ".hidden.jpg", "jpg", "notes.txt"] {
            assert!(!user_picture(n), "{n}");
        }
    }

    #[test]
    fn newest_first_then_the_bundled_by_name() {
        let d = std::env::temp_dir().join(format!("tessera-ui-picker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (mine, b1, b2) = (d.join("mine"), d.join("b1"), d.join("b2"));
        for x in [&mine, &b1, &b2] {
            std::fs::create_dir_all(x).unwrap();
        }
        std::fs::create_dir_all(mine.join("folder.jpg")).unwrap();
        let t = std::time::SystemTime::now();
        for (n, age) in [("old.jpg", 300), ("new.png", 10), ("mid.webp", 100), ("skip.gif", 0)] {
            let f = std::fs::File::create(mine.join(n)).unwrap();
            f.set_modified(t - std::time::Duration::from_secs(age)).unwrap();
        }
        for (dir, n) in [(&b2, "a.jpg"), (&b1, "c.jpg"), (&b1, "b.jpg"), (&b1, "x.png")] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        let names: Vec<String> = list(mine.to_str().unwrap(), &[b1.clone(), b2.clone()]).iter().map(|p| p.rsplit('/').next().unwrap().to_string()).collect();
        assert_eq!(names, vec!["new.png", "mid.webp", "old.jpg", "a.jpg", "b.jpg", "c.jpg"]);
        // no folder of your own: the bundled ones
        assert_eq!(list("/nonexistent/folder", &[b2.clone()]).len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_chosen_one_found_through_links() {
        let d = std::env::temp_dir().join(format!("tessera-ui-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let real = d.join("real.jpg");
        std::fs::write(&real, b"x").unwrap();
        let link = d.join("link.jpg");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let files = vec!["/elsewhere/a.jpg".to_string(), real.to_string_lossy().into_owned()];
        assert_eq!(index_of(&files, real.to_str().unwrap()), Some(1));
        assert_eq!(index_of(&files, link.to_str().unwrap()), Some(1));
        assert_eq!(index_of(&files, "/elsewhere/a.jpg"), Some(0));
        assert_eq!(index_of(&files, "/nonexistent.jpg"), None);
        assert_eq!(index_of(&files, ""), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}

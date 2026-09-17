//! File type grouping for the treemap colours and by-type breakdown.

use crate::model::FileKindGroup;

/// Lower-cased extension without the dot; `None` for dotfiles and implausible suffixes.
pub fn extension(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() || ext.len() > 12 {
        return None;
    }
    if !ext
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

pub fn kind_for_extension(ext: Option<&str>) -> FileKindGroup {
    use FileKindGroup::*;
    let Some(ext) = ext else {
        return Other;
    };
    match ext {
        "png" | "jpg" | "jpeg" | "gif" | "heic" | "heif" | "webp" | "tif" | "tiff" | "bmp"
        | "svg" | "ico" | "icns" | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "psd"
        | "avif" => Image,
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" | "wmv" | "flv" | "mpg" | "mpeg" | "3gp"
        | "mts" | "m2ts" => Video,
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "opus" | "aiff" | "aif" | "wma"
        | "alac" | "caf" => Audio,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "pages" | "numbers" | "key"
        | "txt" | "md" | "rtf" | "odt" | "ods" | "odp" | "epub" | "tex" => Document,
        "zip" | "gz" | "tgz" | "tar" | "bz2" | "xz" | "7z" | "rar" | "zst" | "lz4" | "lzma"
        | "cab" | "jar" | "whl" | "crate" | "nupkg" => Archive,
        "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "py" | "c" | "h" | "cc" | "cpp"
        | "hpp" | "java" | "kt" | "go" | "swift" | "m" | "mm" | "rb" | "php" | "cs" | "html"
        | "css" | "scss" | "vue" | "svelte" | "sh" | "zsh" | "ps1" | "lua" | "dart" | "scala"
        | "zig" | "hs" | "ex" | "exs" | "sql" | "wasm" | "map" | "o" | "rlib" | "rmeta" | "pyc"
        | "class" => Code,
        "app" | "exe" | "msi" | "pkg" | "deb" | "rpm" | "appimage" | "apk" | "ipa" | "msix"
        | "flatpak" | "snap" => Application,
        "dmg" | "iso" | "img" | "vmdk" | "vdi" | "qcow2" | "vhd" | "vhdx" | "sparseimage"
        | "sparsebundle" | "ova" => DiskImage,
        "json" | "csv" | "tsv" | "xml" | "yaml" | "yml" | "toml" | "sqlite" | "sqlite3" | "db"
        | "parquet" | "arrow" | "npy" | "npz" | "h5" | "pb" | "onnx" | "safetensors" | "gguf"
        | "bin" | "dat" | "log" | "pack" | "idx" | "mmdb" | "realm" => Data,
        "dylib" | "so" | "dll" | "sys" | "kext" | "framework" | "plist" | "ttf" | "otf"
        | "woff" | "woff2" | "a" | "lib" | "drv" | "cat" | "mui" | "car" | "nib" | "strings" => {
            System
        }
        _ => Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_extensions() {
        assert_eq!(extension("Movie.MOV").as_deref(), Some("mov"));
        assert_eq!(extension("archive.tar.gz").as_deref(), Some("gz"));
        assert_eq!(extension(".bashrc"), None);
        assert_eq!(extension("README"), None);
        assert_eq!(extension("weird.name with space"), None);
    }

    #[test]
    fn groups_common_types() {
        assert_eq!(kind_for_extension(Some("heic")), FileKindGroup::Image);
        assert_eq!(kind_for_extension(Some("dmg")), FileKindGroup::DiskImage);
        assert_eq!(kind_for_extension(Some("gguf")), FileKindGroup::Data);
        assert_eq!(kind_for_extension(Some("dylib")), FileKindGroup::System);
        assert_eq!(kind_for_extension(None), FileKindGroup::Other);
    }
}

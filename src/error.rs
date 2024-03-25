use thiserror::Error;

#[derive(Debug, Error)]
pub enum TranslatorError {
  #[error("[ReadirError]: An error has ocurred while trying to read directory {directory_path}.\nDetail: {detail}")]
  ReadDirError {
    directory_path: String,
    detail: String,
  },
  #[error("[BundleResourceError]: An error has ocurred while adding a resource to bundle.")]
  BundleResourceError,
  #[error("[DirEntryError]: An error has ocurred while reading directory data.\nDetail: {detail}")]
  DirEntryError { detail: String },
  #[error("[NoDefaultLanguage]: Default language has not been added to the translator")]
  NoDefaultLanuage,
}

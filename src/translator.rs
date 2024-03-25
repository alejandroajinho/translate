use crate::{MessageTranslator, TranslatorError, TRANSLATION_FAILED};

use std::{
  borrow::Cow,
  collections::HashMap,
  fs::{self, DirEntry},
  io::Error as IoError,
  marker::PhantomData,
};

use fluent_bundle::{bundle::FluentBundle, FluentMessage, FluentResource};
use intl_memoizer::concurrent::IntlLangMemoizer;
use tracing::{debug, info, trace, warn};
use unic_langid::LanguageIdentifier;

pub type Bundle = FluentBundle<FluentResource, IntlLangMemoizer>;
type DirectoryData = (DirEntry, String);

pub trait TranslationKey {
  fn as_str(&self) -> &'static str;
}

pub trait Language {
  fn as_str(&self) -> &'static str;
}

pub struct Translator<LanguageGeneric, TranslationKeyGeneric>
where
  LanguageGeneric: Language,
{
  translations: HashMap<String, Bundle>,
  default_language: &'static str,
  phantom: PhantomData<(LanguageGeneric, TranslationKeyGeneric)>,
}

impl<LanguageGeneric, TranslationKeyGeneric> Translator<LanguageGeneric, TranslationKeyGeneric>
where
  LanguageGeneric: Language,
  TranslationKeyGeneric: TranslationKey,
{
  /// ### Description
  /// Creates a new translator, loading translations from directory path and setting default language.
  /// ### Usage
  /// ```
  /// use translate::{Translator, TranslationKey, Language};
  ///
  /// enum TranslationKeys {
  ///   Hello,
  ///   HelloWithArguments
  /// }
  ///
  /// enum Languages {
  ///   Spanish,
  ///   English
  /// }
  ///
  /// impl TranslationKey for TranslationKeys {
  ///   as_str(&self) -> &'static str {
  ///     match self {
  ///       Self::Hello => "hello",
  ///       Self::HelloWithArguments => "hello_with_arguments"
  ///     }
  ///   }
  /// }
  ///
  /// impl Language for Languages {
  ///   as_str(&self) -> &'static str {
  ///     match self {
  ///       Self::Spanish => "es-ES",
  ///       Self::English = > "en-US"
  ///      }
  ///   }
  /// }
  ///
  /// let path = "path/to/translations";
  ///
  /// let translator = Translator::<Languages, TranslationKeys>::new(path, Languages::English)?;
  /// ```
  pub fn new(
    directory_path: &str,
    default_language: &LanguageGeneric,
  ) -> Result<Translator<LanguageGeneric, TranslationKeyGeneric>, TranslatorError> {
    info!("Loading langauges...");

    let translations_directory =
      fs::read_dir(directory_path).map_err(|error| TranslatorError::ReadDirError {
        directory_path: directory_path.to_string(),
        detail: error.to_string(),
      })?;

    let mut translations = HashMap::new();

    for directory_entry_result in translations_directory {
      let directory_data = get_directory_data(directory_entry_result);

      let Some(directory_data) = directory_data else {
        continue;
      };

      let (directory, directory_name) = directory_data;

      let Ok(language_identifier) = directory_name.parse::<LanguageIdentifier>() else {
        warn!(
          "Ignoring {} as it is not a valid langugae identifier",
          directory_name
        );
        continue;
      };

      debug!("Loading translations for {}", directory_name);

      let languages = vec![language_identifier];
      let mut bundle = Bundle::new_concurrent(languages);

      let language_directory =
        fs::read_dir(directory.path()).map_err(|error| TranslatorError::ReadDirError {
          directory_path: directory.path().to_string_lossy().to_string(),
          detail: error.to_string(),
        })?;

      for file_entry_result in language_directory {
        let file_data = get_file_data(file_entry_result);

        let Some(file_data) = file_data else {
          continue;
        };

        let (content, file_name) = file_data;

        let resource = FluentResource::try_new(content);

        match resource {
          Ok(resource) => {
            let bundle_result = bundle.add_resource(resource);
            if bundle_result.is_err() {
              warn!("Could not add resource from file {file_name} from language {directory_name}");
            }
          }
          Err(_) => {
            warn!("Corrupt entry found in file {file_name} from langauge {directory_name}")
          }
        };
      }

      translations.insert(directory_name, bundle);
    }

    info!("Successfully loaded {} languages", translations.len());

    if !translations.contains_key(default_language.as_str()) {
      return Err(TranslatorError::NoDefaultLanuage);
    }

    Ok(Translator {
      translations,
      default_language: default_language.as_str(),
      phantom: PhantomData,
    })
  }

  pub fn get_message<'lifetime>(
    &'lifetime self,
    language: &LanguageGeneric,
    key: &TranslationKeyGeneric,
  ) -> (Option<FluentMessage>, &'lifetime Bundle) {
    let translation_key = key.as_str();
    let default_language = self.default_language;

    let translation_info_optional = self.translations.get(language.as_str());

    let translation_info = if let Some(translation_info) = translation_info_optional {
      (translation_info, language.as_str())
    } else {
      warn!(
        "Tried to translate to an unknown language {}, falling back to {default_language}",
        language.as_str()
      );
      (
        self.translations.get(default_language).unwrap(),
        default_language,
      )
    };

    let (translations, language) = translation_info;
    let mut message = translations.get_message(translation_key);

    if message.is_none() && language != default_language {
      message = self
        .translations
        .get(default_language)
        .unwrap()
        .get_message(translation_key);
    }

    (message, translations)
  }

  /// ### Description
  /// Translate text that takes no arguments
  /// ### Usage
  /// ```
  /// ...
  /// let language = Languages::Espanish;
  /// let key = TranslationKeys::Hello;
  ///
  /// let message = translator.translate_without_arguments(language, key);
  ///
  /// println!("{message}");
  /// ```
  pub fn translate_without_arguments(
    &self,
    language: &LanguageGeneric,
    key: TranslationKeyGeneric,
  ) -> Cow<str> {
    let (message, bundle) = self.get_message(language, &key);

    let Some(message) = message else {
      warn!(
        "Tried to translate to a non existing language key: {}",
        key.as_str()
      );
      return Cow::Borrowed(TRANSLATION_FAILED);
    };

    let mut errors = Vec::new();
    let Some(message_value) = message.value() else {
      warn!("An error has ocurred while tring to get meesage value");
      return Cow::Borrowed(TRANSLATION_FAILED);
    };

    let translated = bundle.format_pattern(message_value, None, &mut errors);

    if errors.is_empty() {
      translated
    } else {
      warn!(
        "Translation failure(s) when translating {}: {:?}",
        key.as_str(),
        errors
      );
      Cow::Borrowed(TRANSLATION_FAILED)
    }
  }

  /// ### Description
  /// Translates text that takes arguments
  /// ### Usage
  /// ```
  /// ...
  /// let language = Languages::Espanish;
  /// let key = TranslationKeys::HelloWithArguments;
  ///
  /// let message = translator.translate(arguments, key).add_argument("name", "Alex");
  /// let built_message = message.buld();
  ///
  /// println!("{bult_message}");
  /// ```
  pub fn translate(
    &self,
    language: &LanguageGeneric,
    key: TranslationKeyGeneric,
  ) -> MessageTranslator<TranslationKeyGeneric> {
    let (message, bundle) = self.get_message(language, &key);

    MessageTranslator {
      key,
      bundle,
      message,
      args: Default::default(),
    }
  }
}

fn get_directory_data(directory_entry_result: Result<DirEntry, IoError>) -> Option<DirectoryData> {
  let Ok(directory) = directory_entry_result else {
    warn!("One directory could not be read");
    return None;
  };

  let directory_name = directory.file_name().to_string_lossy().to_string();

  let Ok(is_dir) = is_directory(&directory) else {
    warn!("Could not check if {directory_name} is a directory");
    return None;
  };

  if is_dir {
    return Some((directory, directory_name));
  }

  None
}

fn is_directory(directory: &DirEntry) -> Result<bool, TranslatorError> {
  let file_type = directory
    .file_type()
    .map_err(|error| TranslatorError::DirEntryError {
      detail: error.to_string(),
    })?;

  Ok(file_type.is_dir())
}

fn get_file_data(file_entry_result: Result<DirEntry, IoError>) -> Option<(String, String)> {
  let Ok(file) = file_entry_result else {
    warn!("One file could not be read");
    return None;
  };

  let file_name = file.file_name().to_string_lossy().to_string();

  trace!("Loading file {file_name}");

  let file_data_result = fs::read_to_string(file.path());

  match file_data_result {
    Ok(file_content) => Some((file_content, file_name)),
    Err(error) => {
      warn!("Could not read file {file_name} because of the following error: {error}");
      None
    }
  }
}

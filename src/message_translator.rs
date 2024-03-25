use crate::translator::{Bundle, TranslationKey};

use fluent_bundle::{FluentArgs, FluentMessage, FluentValue};
use std::borrow::Cow;
use tracing::warn;

pub const TRANSLATION_FAILED: &str = "An error has ocurred while trying to translate the message";

pub struct MessageTranslator<'lifetime, TranslationKeyGeneric>
where
  TranslationKeyGeneric: TranslationKey,
{
  pub key: TranslationKeyGeneric,
  pub bundle: &'lifetime Bundle,
  pub message: Option<FluentMessage<'lifetime>>,
  pub args: Option<FluentArgs<'lifetime>>,
}

impl<'lifetime, Key> MessageTranslator<'lifetime, Key>
where
  Key: TranslationKey,
{
  pub fn add_argument<P>(mut self, key: &'lifetime str, value: P) -> Self
  where
    P: Into<FluentValue<'lifetime>>,
  {
    let mut args = self.args.unwrap_or(FluentArgs::new());
    args.set(key, value.into());
    self.args = Some(args);
    self
  }

  pub fn build(&self) -> Cow<str> {
    let mut errors = Vec::new();

    let Some(message) = &self.message else {
      warn!(
        "Tried to translate a non existing language key: {}",
        self.key.as_str()
      );
      return Cow::Borrowed(TRANSLATION_FAILED);
    };

    let Some(message_value) = message.value() else {
      return Cow::Borrowed(TRANSLATION_FAILED);
    };

    let translated = self
      .bundle
      .format_pattern(message_value, self.args.as_ref(), &mut errors);

    if errors.is_empty() {
      translated
    } else {
      warn!(
        "Translation failure(s) when translating {} with args {:?}: {:?}",
        self.key.as_str(),
        self.args,
        errors
      );

      Cow::Borrowed(TRANSLATION_FAILED)
    }
  }
}

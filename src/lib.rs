#![deny(clippy::all)]

mod bindings;
mod emit;
mod error;
mod io;
mod ir;
mod options;
mod parse;
mod pipeline;
pub mod plan;
mod result;
#[cfg(test)]
mod test_support;

use napi_derive::napi;

pub use crate::bindings::{
  EmitTarget, GenerateErrorPayload, GenerateOptions, GenerateOutcome, GenerateResult,
};
use crate::bindings::{map_failure, map_generate_result, map_panic};
pub use crate::options::{GenerateConfig, MappedType};
pub use crate::pipeline::execute_generate;
pub use crate::plan::naming::NamingConfig;

/// Native export consumed only by `lib/index.js` and `lib/browser.js`.
/// Never throws: fatals and panics travel as `GenerateOutcome.error`.
#[napi(js_name = "generateNative")]
pub fn generate(options: GenerateOptions) -> GenerateOutcome {
  let config = GenerateConfig::from(options);
  // `AssertUnwindSafe` is sound: `config` is consumed by value and nothing
  // the closure touches is observed after an unwind.
  let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| execute_generate(config)));
  match outcome {
    Ok(Ok(result)) => GenerateOutcome {
      result: Some(map_generate_result(result)),
      error: None,
    },
    Ok(Err(failure)) => GenerateOutcome {
      result: None,
      error: Some(map_failure(failure)),
    },
    Err(panic_payload) => GenerateOutcome {
      result: None,
      error: Some(map_panic(panic_payload)),
    },
  }
}

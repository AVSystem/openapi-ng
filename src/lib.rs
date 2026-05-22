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

use napi::Env;
use napi_derive::napi;

pub use crate::bindings::{EmitTarget, GenerateErrorPayload, GenerateOptions, GenerateResult};
use crate::bindings::{map_failure, map_generate_result, map_panic};
pub use crate::options::{GenerateConfig, MappedType};
pub use crate::pipeline::execute_generate;
pub use crate::plan::naming::NamingConfig;

#[napi(js_name = "generate")]
pub fn generate(env: Env, options: GenerateOptions) -> napi::Result<GenerateResult> {
  let config = GenerateConfig::from(options);
  // `catch_unwind` ensures a Rust panic inside the pipeline becomes a
  // typed `E_UNEXPECTED` GenerateError rather than aborting the host
  // Node process. `AssertUnwindSafe` is sound here because `config`
  // is consumed by value and nothing the closure touches is observed
  // after the unwind path.
  let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| execute_generate(config)));
  match outcome {
    Ok(Ok(result)) => Ok(map_generate_result(result)),
    Ok(Err(failure)) => Err(map_failure(failure, env)),
    Err(panic_payload) => Err(map_panic(panic_payload, env)),
  }
}

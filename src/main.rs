use std::time::Instant;

use openapi_ng::{EmitTarget, GenerateConfig, MappedType, execute_generate};

fn main() {
  if do_it().is_err() {
    std::process::exit(1);
  }
}

fn do_it() -> Result<(), Box<dyn std::error::Error>> {
  let input_path = std::env::args()
    .nth(1)
    .unwrap_or_else(|| String::from("./.something/internal.json"));
  let output_path = std::env::args()
    .nth(2)
    .unwrap_or_else(|| String::from("./.something/output"));

  let mapped_types = vec![MappedType {
    schema: String::from("GeoJSON"),
    import: String::from("geojson"),
    ty: String::from("GeoJSON"),
    alias: Some(String::from("NativeGeoJSON")),
  }];
  let start = Instant::now();
  execute_generate(GenerateConfig {
    input_path: Some(input_path),
    input_contents: None,
    display_path: None,
    input_format: None,
    output_path: Some(output_path),
    emit: [EmitTarget::Models, EmitTarget::Angular]
      .into_iter()
      .collect(),
    mapped_types,
    response_type_mapping: Vec::new(),
    naming_options: None,
    naming: openapi_ng::plan::naming::NamingConfig::default(),
  })
  .map(|_| println!("Generation completed in {:?}", start.elapsed()))
  .map_err(|f| -> Box<dyn std::error::Error> { f.fatal.message.into() })
}

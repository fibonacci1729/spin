use anyhow::{Context, Result};
use spin_componentize::componentize_if_necessary;
use std::mem;
use wasmparser::{Parser, Validator, WasmFeatures, Payload};
use wasm_encoder::{ComponentSectionId, Encode, RawSection, Section, ComponentSection};

const ISOLATE_INTERFACES: &[&str] = &[
    "wasi:filesystem/preopens@0.2.0-rc-2023-10-18",
    "wasi:cli/environment@0.2.0-rc-2023-10-18",
];

/// Isolate any required imports by prefixing the interface name with a prefix.
pub fn imports(bytes: &[u8], prefix: &str) -> Result<Vec<u8>> {
    let bytes = componentize_if_necessary(&bytes).context("failed to componentize")?;

    let mut output = Vec::new();
    let mut stack = Vec::new();
    let mut depth = 0;

    for payload in Parser::new(0).parse_all(&bytes) {
        let payload = payload?;

        // Track nesting depth, so that we don't mess with inner producer sections:
        match payload {
            Payload::Version { encoding, .. } => {
                output.extend_from_slice(match encoding {
                    wasmparser::Encoding::Component => &wasm_encoder::Component::HEADER,
                    wasmparser::Encoding::Module => &wasm_encoder::Module::HEADER,
                });
            }
            Payload::ModuleSection { .. } | Payload::ComponentSection { .. } => {
                stack.push(mem::take(&mut output));
                depth += 1;
                continue;
            }
            Payload::End { .. } => {
                depth -= 1;

                let mut parent = match stack.pop() {
                    Some(c) => c,
                    None => break,
                };
                if output.starts_with(&wasm_encoder::Component::HEADER) {
                    parent.push(ComponentSectionId::Component as u8);
                    output.encode(&mut parent);
                } else {
                    parent.push(ComponentSectionId::CoreModule as u8);
                    output.encode(&mut parent);
                }
                output = parent;
            }
            _ => {}
        }

        match &payload {
            Payload::ComponentImportSection(section) => {
                if depth == 0 {
                    for result in section.clone() {
                        let import = result?;
                        let name = import.name.0;
                        if ISOLATE_INTERFACES.contains(&name) {
                            let isolated = format!("{prefix}-{name}");
                            encode_import(&mut output, &isolated, import.ty);
                            println!("rewrote import {name} -> {isolated}");
                        } else {
                            encode_import(&mut output, &name, import.ty);
                        }
                    }
                    continue;
                }
            }
            _ => {}
        }
        if let Some((id, range)) = payload.as_section() {
            RawSection {
                id,
                data: &bytes[range],
            }
            .append_to(&mut output);
        }
    }

    Validator::new_with_features(WasmFeatures { component_model: true, ..Default::default() })
        .validate_all(&output)
        .context("failed to validate output component")?;    

    Ok(output)
}

fn convert_wp_component_type_ref_to_we(ty: wasmparser::ComponentTypeRef) -> wasm_encoder::ComponentTypeRef {
    match ty {
        wasmparser::ComponentTypeRef::Module(idx) => wasm_encoder::ComponentTypeRef::Module(idx),
        wasmparser::ComponentTypeRef::Func(idx) => wasm_encoder::ComponentTypeRef::Func(idx),
        wasmparser::ComponentTypeRef::Value(ty) => wasm_encoder::ComponentTypeRef::Value(convert_wp_component_val_type_to_we(ty)),
        wasmparser::ComponentTypeRef::Type(ty) => wasm_encoder::ComponentTypeRef::Type(convert_wp_type_bounds_to_we(ty)),
        wasmparser::ComponentTypeRef::Instance(idx) => wasm_encoder::ComponentTypeRef::Instance(idx),
        wasmparser::ComponentTypeRef::Component(idx) => wasm_encoder::ComponentTypeRef::Component(idx),
    }
}
fn convert_wp_component_val_type_to_we(ty: wasmparser::ComponentValType) -> wasm_encoder::ComponentValType {
    match ty {
        wasmparser::ComponentValType::Primitive(ty) => wasm_encoder::ComponentValType::Primitive(convert_wp_primitive_val_type_to_we(ty)),
        wasmparser::ComponentValType::Type(idx) => wasm_encoder::ComponentValType::Type(idx),
    }
}
fn convert_wp_primitive_val_type_to_we(ty: wasmparser::PrimitiveValType) -> wasm_encoder::PrimitiveValType {
    match ty {
        wasmparser::PrimitiveValType::Bool => wasm_encoder::PrimitiveValType::Bool,
        wasmparser::PrimitiveValType::S8 => wasm_encoder::PrimitiveValType::S8,
        wasmparser::PrimitiveValType::U8 => wasm_encoder::PrimitiveValType::U8,
        wasmparser::PrimitiveValType::S16 => wasm_encoder::PrimitiveValType::S16,
        wasmparser::PrimitiveValType::U16 => wasm_encoder::PrimitiveValType::U16,
        wasmparser::PrimitiveValType::S32 => wasm_encoder::PrimitiveValType::S32,
        wasmparser::PrimitiveValType::U32 => wasm_encoder::PrimitiveValType::U32,
        wasmparser::PrimitiveValType::S64 => wasm_encoder::PrimitiveValType::S64,
        wasmparser::PrimitiveValType::U64 => wasm_encoder::PrimitiveValType::U64,
        wasmparser::PrimitiveValType::Float32 => wasm_encoder::PrimitiveValType::Float32,
        wasmparser::PrimitiveValType::Float64 => wasm_encoder::PrimitiveValType::Float64,
        wasmparser::PrimitiveValType::Char => wasm_encoder::PrimitiveValType::Char,
        wasmparser::PrimitiveValType::String => wasm_encoder::PrimitiveValType::String,
    }
}
fn convert_wp_type_bounds_to_we(ty: wasmparser::TypeBounds) -> wasm_encoder::TypeBounds {
    match ty {
        wasmparser::TypeBounds::Eq(idx) => wasm_encoder::TypeBounds::Eq(idx),
        wasmparser::TypeBounds::SubResource => wasm_encoder::TypeBounds::SubResource,
    }
}
fn encode_import(output: &mut Vec<u8>, name: &str, ty: wasmparser::ComponentTypeRef) {
    let mut section = wasm_encoder::ComponentImportSection::new();
    output.push(section.id());
    let ty: wasm_encoder::ComponentTypeRef = convert_wp_component_type_ref_to_we(ty);
    section.import(&name, ty);
    section.encode(output);
}
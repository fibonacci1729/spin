use anyhow::{bail, Context, Result};
use indexmap::{IndexMap, IndexSet};
use spin_app::{App, AppComponent};
use spin_common::url::parse_file_url;
use std::{fs, marker::PhantomData};
use wasm_compose::graph::{CompositionGraph, Component, ComponentId, InstanceId, ImportIndex, ExportIndex, EncodeOptions};
use wasmparser::{ComponentExternalKind, ComponentTypeRef, types::ComponentInstanceTypeId};
use crate::isolate;

pub struct SpinComposer<'a> {
    composition_graph: CompositionGraph<'a>,
    components: IndexMap<String, ComponentId>,
    instances: IndexMap<String, InstanceId>,
}

impl<'a> SpinComposer<'a> {
    pub fn new() -> Self {
       SpinComposer {
            composition_graph: CompositionGraph::new(),
            components: IndexMap::new(),
            instances: IndexMap::new(),
        }
    }

    pub fn compose<L>(mut self, app_component: &AppComponent<'_, L>) -> Result<Vec<u8>> {
        let instance_id = self.add_component(app_component)?;
        self.add_dependencies(&mut IndexSet::new(), app_component)?;
        self.build_composition(app_component)?;

        self.composition_graph.unify_imported_resources();

        let bytes = self.composition_graph.encode(EncodeOptions {
            define_components: true,
            export: Some(instance_id),
            .. Default::default()
        }).context(format!("encoding composed component {:?}", app_component.id()))?;

        // let text = wasmprinter::print_bytes(&bytes)?;
        // println!("{text}");

        // Ok(bytes)
        Ok(bytes)
    }

    /// Adds a component of the given name to the graph.
    ///
    /// If a component with the given name already exists, its id is returned.
    /// Returns `Ok(None)` if a matching component cannot be found.
    fn add_component<L>(&mut self, app_component: &AppComponent<'_, L>) -> Result<InstanceId> {
        let app_component_id = app_component.id().to_string();

        if let Some((_component_id, _)) = self.composition_graph.get_component_by_name(&app_component_id) {
            return Ok(self.instances.get(&app_component_id).copied().unwrap());
            // return Ok(component_id);
        }

        println!("adding component `{}`", app_component_id);

        let source = app_component
            .source()
            .content
            .source
            .as_ref()
            .context("AppComponent missing source field")?;
    
        let path = parse_file_url(source)?;

        let bytes = fs::read(&path).with_context(|| {
            format!(
                "failed to read component `{}` source from disk at path '{}'",
                app_component_id,
                path.display(),
            )
        })?;
        
        // TODO: derive which imports to isolate based on settings provided in `spin.toml`
        let isolated = isolate::imports(&bytes, &app_component_id)?;

        let def_component = Component::from_bytes(&app_component_id, isolated)?;
        let component_id = self.composition_graph.add_component(def_component)?;

        assert!(self.components.insert(app_component_id.clone(), component_id).is_none());

        let instance_id = self
            .composition_graph
            .instantiate(component_id)
            .context(format!("instantiating component `{}`", app_component_id.clone()))?;

        assert!(self.instances.insert(app_component_id, instance_id).is_none());
        
        Ok(instance_id)
        // Ok(component_id)
    }

    fn add_dependencies<L>(&mut self, visited: &mut IndexSet<String>, c: &AppComponent<'_, L>) -> Result<()> {
        if !visited.insert(c.id().to_string()) {
            anyhow::bail!("cycle");
        }
        for (name, import) in c.imports() {
            let Some(dep) = c.app.get_component(&import.component) else {
                anyhow::bail!("component {} dependency {} for import {} is not defined in `spin.toml`",
                    c.id(),
                    import.component,
                    name,
                );
            };
            self.add_component(&dep).with_context(|| format!(
                "adding component {:?} dependency {:?}", 
                c.id(), 
                dep.id(),
            ))?;
            self.add_dependencies(visited, &dep)?;
            visited.remove(dep.id());
        }

        Ok(())
    }

    fn build_composition<L>(&mut self, c: &AppComponent<'_, L>) -> Result<()> {
        println!("building `{}`", c.id());

        for (name, import) in c.imports() {
            let dep = c.app.get_component(&import.component).unwrap();
            self.build_composition(&dep)?;

            self.connect(
                dep.id(),
                import.export.as_deref(),
                c.id(),
                &name,
            )?;
        }

        Ok(())
    }

    fn connect(&mut self, source: &str, source_export: Option<&str>, target: &str, target_import: &str) -> Result<()> {
        let source_instance_id = self.instances.get(source).copied().unwrap();
        let target_instance_id = self.instances.get(target).copied().unwrap();

        let target_import = self.resolve_import(target, target_import)?;

        let source_export = if let Some(export_name) = source_export {
            self.resolve_export(source, export_name).map(Option::Some)?
        } else {
            None
        };

        self.composition_graph.connect(
            source_instance_id,
            source_export,
            target_instance_id,
            target_import,
        )
    }

    // fn resolve_import(&self, component: &str, import_name: &str) -> Result<(ImportIndex, ComponentInstanceTypeId)> {
    fn resolve_import(&self, component: &str, import_name: &str) -> Result<ImportIndex> {
        let (_, def_component) = self
            .composition_graph
            .get_component_by_name(component)
            .unwrap();

        match def_component.import_by_name(import_name) {
            Some((import_index, _import_ty)) => {
                // def_component
                //     .types()
                //     .component_any_type_at(index)
                //     .unwrap_instance()

                Ok(import_index)
            }
            // Some((_, _)) => {
            //     unreachable!("should not have an instance import ref to a non-instance import");
            // }
            None => {
                bail!("component `{component}` does not export an instance named `{import_name}`"); 
            }
        }
    }

    fn resolve_export(&self, component: &str, export_name: &str) -> Result<ExportIndex> {
        let (_, dep_component) = self
            .composition_graph
            .get_component_by_name(component)
            .unwrap();

        match dep_component.export_by_name(export_name) {
            Some((export_index, kind, _index)) if kind == ComponentExternalKind::Instance => {
                // let result = self.composition_graph.try_connection()
                // if self.graph.try_connection(
                //     component_id,
                //     ComponentEntityType::Instance(export_ty),
                //     component.types(),
                //     ComponentEntityType::Instance(ty),
                //     types,
                // ) {
                //     Ok(export_index)
                // } else {
                //     bail!(
                //         "component `{path}` exports an instance named `{export}` \
                //          but it is not compatible with import `{arg_name}` \
                //          of component `{dependent_path}`",
                //         path = component.path().unwrap().display(),
                //         dependent_path = dependent_path.display(),
                //     )
                // }

                Ok(export_index)
            }
            _ => {
                // TODO: find compatible instance export in component
                bail!("component `{component}` does not export an instance named `{export_name}`");
            }
        }
    }
}
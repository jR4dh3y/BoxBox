use ash::vk;
use naga::back::spv;
use std::{ffi, mem, str};

const DUMP_PREFIX: Option<&str> = None;

struct CompiledShader<'a> {
    vk_module: vk::ShaderModule,
    _entry_point: ffi::CString,
    create_info: vk::PipelineShaderStageCreateInfo<'a>,
    attribute_mappings: Vec<crate::VertexAttributeMapping>,
    wg_size: [u32; 3],
}

impl super::Context {
    fn make_spv_options(&self, data_layouts: &[&crate::ShaderDataLayout]) -> spv::Options {
        // collect all the array bindings into overrides
        let mut binding_map = spv::BindingMap::default();
        for (group_index, layout) in data_layouts.iter().enumerate() {
            for (binding_index, &(_, binding)) in layout.bindings.iter().enumerate() {
                match binding {
                    crate::ShaderBinding::TextureArray { count }
                    | crate::ShaderBinding::BufferArray { count } => {
                        let rb = naga::ResourceBinding {
                            group: group_index as u32,
                            binding: binding_index as u32,
                        };
                        binding_map.insert(
                            rb,
                            spv::BindingInfo {
                                binding_array_size: Some(count),
                            },
                        );
                    }
                    _ => {}
                }
            }
        }

        spv::Options {
            lang_version: match self.device.ray_tracing {
                // Required for ray queries
                Some(_) => (1, 4),
                None => (1, 3),
            },
            flags: self.naga_flags,
            binding_map,
            capabilities: None,
            bounds_check_policies: naga::proc::BoundsCheckPolicies::default(),
            zero_initialize_workgroup_memory: spv::ZeroInitializeWorkgroupMemoryMode::None,
            force_loop_bounding: false,
            debug_info: None,
        }
    }

    fn spv_cache_key(
        sf: &crate::ShaderFunction,
        naga_options: &spv::Options,
        group_layouts: &[&crate::ShaderDataLayout],
        vertex_fetch_states: &[crate::VertexFetchState],
    ) -> [u64; 2] {
        let mut h = super::shader_cache::Hasher128::new();
        h.u64(sf.shader.source_hash[0]);
        h.u64(sf.shader.source_hash[1]);
        h.bytes(sf.entry_point.as_bytes());
        let mut constants: Vec<(&String, &f64)> = sf.constants.iter().collect();
        constants.sort_by(|a, b| a.0.cmp(b.0));
        h.u32(constants.len() as u32);
        for (name, value) in constants {
            h.bytes(name.as_bytes());
            h.u64(value.to_bits());
        }
        h.u8(naga_options.lang_version.0);
        h.u8(naga_options.lang_version.1);
        h.u32(naga_options.flags.bits());
        h.u32(group_layouts.len() as u32);
        for layout in group_layouts {
            h.u32(layout.bindings.len() as u32);
            for &(name, binding) in &layout.bindings {
                h.bytes(name.as_bytes());
                match binding {
                    crate::ShaderBinding::Texture => h.u8(0),
                    crate::ShaderBinding::TextureArray { count } => {
                        h.u8(1);
                        h.u32(count);
                    }
                    crate::ShaderBinding::Sampler => h.u8(2),
                    crate::ShaderBinding::Buffer => h.u8(3),
                    crate::ShaderBinding::BufferArray { count } => {
                        h.u8(4);
                        h.u32(count);
                    }
                    crate::ShaderBinding::AccelerationStructure => h.u8(5),
                    crate::ShaderBinding::Plain { size } => {
                        h.u8(6);
                        h.u32(size);
                    }
                }
            }
        }
        h.u32(vertex_fetch_states.len() as u32);
        for vf in vertex_fetch_states {
            h.u32(vf.layout.stride);
            h.u8(u8::from(vf.instanced));
            h.u32(vf.layout.attributes.len() as u32);
            for &(name, attr) in &vf.layout.attributes {
                h.bytes(name.as_bytes());
                h.u32(attr.offset);
                h.u8(attr.format as u8);
            }
        }
        h.finish()
    }

    fn load_shader(
        &self,
        sf: crate::ShaderFunction,
        naga_options_base: &spv::Options,
        group_layouts: &[&crate::ShaderDataLayout],
        group_infos: &mut [crate::ShaderDataInfo],
        vertex_fetch_states: &[crate::VertexFetchState],
    ) -> CompiledShader {
        let use_cache = self.shader_debug_path.is_none() && DUMP_PREFIX.is_none();
        let cache_key = use_cache.then(|| {
            Self::spv_cache_key(&sf, naga_options_base, group_layouts, vertex_fetch_states)
        });
        if let Some(cached) = cache_key.and_then(|key| self.shader_cache.get_spv(key)) {
            if cached.group_infos.len() == group_infos.len() {
                for (dst, src) in group_infos.iter_mut().zip(&cached.group_infos) {
                    dst.visibility |= crate::ShaderVisibility::from_bits_truncate(src.visibility);
                    for (dst_acc, &src_acc) in
                        dst.binding_access.iter_mut().zip(&src.binding_access)
                    {
                        *dst_acc |= naga::StorageAccess::from_bits_truncate(src_acc);
                    }
                }
                let vk_info = vk::ShaderModuleCreateInfo::default().code(&cached.spv);
                let vk_module = unsafe {
                    self.device
                        .core
                        .create_shader_module(&vk_info, None)
                        .unwrap()
                };
                let vk_stage = match cached.stage {
                    naga::ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
                    naga::ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
                    naga::ShaderStage::Fragment => vk::ShaderStageFlags::FRAGMENT,
                    _ => panic!("Unsupported shader stage: {:?}", cached.stage),
                };
                let entry_point = ffi::CString::new(sf.entry_point).unwrap();
                let create_info = vk::PipelineShaderStageCreateInfo {
                    stage: vk_stage,
                    module: vk_module,
                    p_name: entry_point.as_ptr(),
                    ..Default::default()
                };
                let attribute_mappings = cached
                    .attribute_mappings
                    .into_iter()
                    .map(|(buffer_index, attribute_index)| crate::VertexAttributeMapping {
                        buffer_index: buffer_index as usize,
                        attribute_index: attribute_index as usize,
                    })
                    .collect();
                return CompiledShader {
                    vk_module,
                    _entry_point: entry_point,
                    create_info,
                    attribute_mappings,
                    wg_size: cached.wg_size,
                };
            }
        }

        let ep_index = sf.entry_point_index();
        let (parsed_module, parsed_info) = sf.shader.module_and_info();
        let ep = &parsed_module.entry_points[ep_index];
        let ep_info = parsed_info.get_entry_point(ep_index);

        let (mut module, module_info) = sf.shader.resolve_constants(&sf.constants);
        let mut stage_infos: Vec<crate::ShaderDataInfo> =
            group_layouts.iter().map(|layout| layout.to_info()).collect();
        crate::Shader::fill_resource_bindings(
            &mut module,
            &mut stage_infos,
            ep.stage,
            ep_info,
            group_layouts,
        );
        for (dst, src) in group_infos.iter_mut().zip(&stage_infos) {
            dst.visibility |= src.visibility;
            for (dst_acc, &src_acc) in dst.binding_access.iter_mut().zip(src.binding_access.iter())
            {
                *dst_acc |= src_acc;
            }
        }
        let attribute_mappings =
            crate::Shader::fill_vertex_locations(&mut module, ep_index, vertex_fetch_states);

        let pipeline_options = spv::PipelineOptions {
            shader_stage: ep.stage,
            entry_point: sf.entry_point.to_string(),
        };
        let file_path;
        let mut naga_options_debug;
        let naga_options = if let Some(ref temp_dir) = self.shader_debug_path {
            use std::{
                fs,
                hash::{DefaultHasher, Hash as _, Hasher as _},
            };
            let mut hasher = DefaultHasher::new();
            sf.shader.source.hash(&mut hasher);
            file_path = temp_dir.join(format!("{}-{:x}.wgsl", sf.entry_point, hasher.finish()));
            log::debug!("Dumping processed shader code to: {}", file_path.display());
            let _ = fs::write(&file_path, &sf.shader.source);

            naga_options_debug = naga_options_base.clone();
            naga_options_debug.debug_info = Some(naga::back::spv::DebugInfo {
                source_code: &sf.shader.source,
                file_name: &file_path,
                //TODO: switch to WGSL once NSight Graphics recognizes it
                language: naga::back::spv::SourceLanguage::GLSL,
            });
            &naga_options_debug
        } else {
            naga_options_base
        };

        let spv =
            spv::write_vec(&module, &module_info, naga_options, Some(&pipeline_options)).unwrap();

        if let Some(key) = cache_key {
            let cached_groups = stage_infos
                .iter()
                .map(|info| super::shader_cache::CachedGroupInfo {
                    visibility: info.visibility.bits(),
                    binding_access: info.binding_access.iter().map(|acc| acc.bits()).collect(),
                })
                .collect();
            let cached_attrs = attribute_mappings
                .iter()
                .map(|mapping| (mapping.buffer_index as u32, mapping.attribute_index as u32))
                .collect();
            self.shader_cache.insert_spv(
                key,
                super::shader_cache::CachedSpv {
                    stage: ep.stage,
                    wg_size: ep.workgroup_size,
                    spv: spv.clone(),
                    group_infos: cached_groups,
                    attribute_mappings: cached_attrs,
                },
            );
        }

        if let Some(dump_prefix) = DUMP_PREFIX {
            let mut file_name = String::new();
            for i in 1.. {
                file_name = format!("{}{}_{:?}{}.spv", dump_prefix, sf.entry_point, ep.stage, i);
                if !std::path::Path::new(&file_name).exists() {
                    break;
                }
            }
            let spv_bytes =
                unsafe { std::slice::from_raw_parts(spv.as_ptr() as *const u8, spv.len() * 4) };
            println!("Dumping {}", file_name);
            std::fs::write(file_name, spv_bytes).unwrap();
        }

        let vk_info = vk::ShaderModuleCreateInfo::default().code(&spv);

        let vk_module = unsafe {
            self.device
                .core
                .create_shader_module(&vk_info, None)
                .unwrap()
        };

        let vk_stage = match ep.stage {
            naga::ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
            naga::ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
            naga::ShaderStage::Fragment => vk::ShaderStageFlags::FRAGMENT,
            _ => panic!("Unsupported shader stage: {:?}", ep.stage),
        };

        let entry_point = ffi::CString::new(sf.entry_point).unwrap();
        let create_info = vk::PipelineShaderStageCreateInfo {
            stage: vk_stage,
            module: vk_module,
            p_name: entry_point.as_ptr(),
            ..Default::default()
        };

        CompiledShader {
            vk_module,
            _entry_point: entry_point,
            create_info,
            attribute_mappings,
            wg_size: ep.workgroup_size,
        }
    }

    fn create_descriptor_set_layout(
        &self,
        layout: &crate::ShaderDataLayout,
        info: &crate::ShaderDataInfo,
    ) -> super::DescriptorSetLayout {
        if info.visibility.is_empty() {
            // we need to have a valid `VkDescriptorSetLayout` regardless
            return super::DescriptorSetLayout {
                raw: unsafe {
                    self.device
                        .core
                        .create_descriptor_set_layout(&Default::default(), None)
                        .unwrap()
                },
                ..Default::default()
            };
        }

        let stage_flags = map_shader_visibility(info.visibility);
        let mut vk_bindings = Vec::with_capacity(layout.bindings.len());
        let mut template_entries = Vec::with_capacity(layout.bindings.len());
        let mut template_offsets = Vec::with_capacity(layout.bindings.len());
        let mut binding_flags = Vec::with_capacity(layout.bindings.len());
        let mut update_offset = 0;
        for (binding_index, (&(_, binding), &access)) in layout
            .bindings
            .iter()
            .zip(info.binding_access.iter())
            .enumerate()
        {
            let (descriptor_type, descriptor_size, descriptor_count, flag) = match binding {
                crate::ShaderBinding::Texture => (
                    if access.is_empty() {
                        vk::DescriptorType::SAMPLED_IMAGE
                    } else {
                        vk::DescriptorType::STORAGE_IMAGE
                    },
                    mem::size_of::<vk::DescriptorImageInfo>(),
                    1u32,
                    vk::DescriptorBindingFlags::empty(),
                ),
                crate::ShaderBinding::TextureArray { count } => (
                    if access.is_empty() {
                        vk::DescriptorType::SAMPLED_IMAGE
                    } else {
                        vk::DescriptorType::STORAGE_IMAGE
                    },
                    mem::size_of::<vk::DescriptorImageInfo>(),
                    count,
                    vk::DescriptorBindingFlags::PARTIALLY_BOUND,
                ),
                crate::ShaderBinding::Sampler => (
                    vk::DescriptorType::SAMPLER,
                    mem::size_of::<vk::DescriptorImageInfo>(),
                    1u32,
                    vk::DescriptorBindingFlags::empty(),
                ),
                crate::ShaderBinding::Buffer => (
                    vk::DescriptorType::STORAGE_BUFFER,
                    mem::size_of::<vk::DescriptorBufferInfo>(),
                    1u32,
                    vk::DescriptorBindingFlags::empty(),
                ),
                crate::ShaderBinding::BufferArray { count } => (
                    vk::DescriptorType::STORAGE_BUFFER,
                    mem::size_of::<vk::DescriptorBufferInfo>(),
                    count,
                    vk::DescriptorBindingFlags::PARTIALLY_BOUND,
                ),
                crate::ShaderBinding::AccelerationStructure => (
                    vk::DescriptorType::ACCELERATION_STRUCTURE_KHR,
                    mem::size_of::<vk::AccelerationStructureKHR>(),
                    1u32,
                    vk::DescriptorBindingFlags::empty(),
                ),
                crate::ShaderBinding::Plain { size } => (
                    vk::DescriptorType::INLINE_UNIFORM_BLOCK_EXT,
                    1,
                    size,
                    vk::DescriptorBindingFlags::empty(),
                ),
            };

            vk_bindings.push(vk::DescriptorSetLayoutBinding {
                binding: binding_index as u32,
                descriptor_type,
                descriptor_count,
                stage_flags,
                ..Default::default()
            });
            template_entries.push(vk::DescriptorUpdateTemplateEntryKHR {
                dst_binding: binding_index as u32,
                dst_array_element: 0,
                descriptor_count,
                descriptor_type,
                offset: update_offset,
                stride: descriptor_size,
            });
            binding_flags.push(flag);
            template_offsets.push(update_offset as u32);
            update_offset += descriptor_size * descriptor_count as usize;
        }

        let mut binding_flags_info =
            vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&binding_flags);
        let set_layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&vk_bindings)
            .push_next(&mut binding_flags_info);
        let raw = unsafe {
            self.device
                .core
                .create_descriptor_set_layout(&set_layout_info, None)
                .unwrap()
        };

        let template_create_info = vk::DescriptorUpdateTemplateCreateInfo::default()
            .descriptor_update_entries(&template_entries)
            .template_type(vk::DescriptorUpdateTemplateTypeKHR::DESCRIPTOR_SET)
            .descriptor_set_layout(raw);
        let update_template = unsafe {
            self.device
                .core
                .create_descriptor_update_template(&template_create_info, None)
                .unwrap()
        };

        super::DescriptorSetLayout {
            raw,
            update_template,
            template_size: update_offset as u32,
            template_offsets: template_offsets.into_boxed_slice(),
        }
    }

    fn create_pipeline_layout(
        &self,
        group_layouts: &[&crate::ShaderDataLayout],
        group_infos: &[crate::ShaderDataInfo],
    ) -> super::PipelineLayout {
        let mut descriptor_set_layouts = Vec::with_capacity(group_layouts.len());
        let mut vk_set_layouts = Vec::with_capacity(group_layouts.len());
        for (&layout, info) in group_layouts.iter().zip(group_infos) {
            let dsl = self.create_descriptor_set_layout(layout, info);
            vk_set_layouts.push(dsl.raw);
            descriptor_set_layouts.push(dsl);
        }

        let vk_info = vk::PipelineLayoutCreateInfo::default().set_layouts(&vk_set_layouts);
        let raw = unsafe {
            self.device
                .core
                .create_pipeline_layout(&vk_info, None)
                .unwrap()
        };

        super::PipelineLayout {
            raw,
            descriptor_set_layouts,
        }
    }

    fn destroy_pipeline_layout(&self, layout: &mut super::PipelineLayout) {
        unsafe {
            self.device
                .core
                .destroy_pipeline_layout(mem::take(&mut layout.raw), None);
        }
        for dsl in layout.descriptor_set_layouts.drain(..) {
            unsafe {
                self.device
                    .core
                    .destroy_descriptor_set_layout(dsl.raw, None);
            }
            if !dsl.is_empty() {
                unsafe {
                    self.device
                        .core
                        .destroy_descriptor_update_template(dsl.update_template, None);
                }
            }
        }
    }
}

#[hidden_trait::expose]
impl crate::traits::ShaderDevice for super::Context {
    type ComputePipeline = super::ComputePipeline;
    type RenderPipeline = super::RenderPipeline;

    fn create_compute_pipeline(&self, desc: crate::ComputePipelineDesc) -> super::ComputePipeline {
        let mut group_infos = desc
            .data_layouts
            .iter()
            .map(|layout| layout.to_info())
            .collect::<Vec<_>>();

        let options = self.make_spv_options(desc.data_layouts);
        let cs = self.load_shader(
            desc.compute,
            &options,
            desc.data_layouts,
            &mut group_infos,
            &[],
        );

        let layout = self.create_pipeline_layout(desc.data_layouts, &group_infos);

        let create_info = vk::ComputePipelineCreateInfo::default()
            .layout(layout.raw)
            .stage(cs.create_info);

        let mut raw_vec = unsafe {
            self.device
                .core
                .create_compute_pipelines(self.pipeline_cache.raw, &[create_info], None)
                .unwrap()
        };
        let raw = raw_vec.pop().unwrap();
        self.save_pipeline_cache();

        unsafe { self.device.core.destroy_shader_module(cs.vk_module, None) };

        if let Some(ref ext) = self.device.shader_info {
            if let Ok(statistics) =
                unsafe { ext.get_shader_info_statistics(raw, vk::ShaderStageFlags::COMPUTE) }
            {
                let ru = &statistics.resource_usage;
                log::info!(
                    "Compute pipeline '{}' uses: {} VGPRs, {} SGPRs",
                    desc.name,
                    ru.num_used_vgprs,
                    ru.num_used_sgprs,
                );
            }
        }

        if !desc.name.is_empty() {
            self.set_object_name(raw, desc.name);
        }
        super::ComputePipeline {
            raw,
            layout,
            wg_size: cs.wg_size,
        }
    }

    fn destroy_compute_pipeline(&self, pipeline: &mut super::ComputePipeline) {
        self.destroy_pipeline_layout(&mut pipeline.layout);
        unsafe {
            self.device.core.destroy_pipeline(pipeline.raw, None);
        }
    }

    fn create_render_pipeline(&self, desc: crate::RenderPipelineDesc) -> super::RenderPipeline {
        let mut group_infos = desc
            .data_layouts
            .iter()
            .map(|layout| layout.to_info())
            .collect::<Vec<_>>();

        let options = self.make_spv_options(desc.data_layouts);
        let vs = self.load_shader(
            desc.vertex,
            &options,
            desc.data_layouts,
            &mut group_infos,
            desc.vertex_fetches,
        );
        let fs = desc.fragment.map(|desc_fragment| {
            self.load_shader(
                desc_fragment,
                &options,
                desc.data_layouts,
                &mut group_infos,
                &[],
            )
        });

        let mut stages = [vs.create_info, vk::PipelineShaderStageCreateInfo::default()];
        let mut stage_count = 1;
        if let Some(ref fs) = fs {
            stages[1] = fs.create_info;
            stage_count += 1;
        }
        let stages = &stages[..stage_count]; // 'dynamic' stack allocated array

        let layout = self.create_pipeline_layout(desc.data_layouts, &group_infos);

        let vertex_buffers = desc
            .vertex_fetches
            .iter()
            .enumerate()
            .map(|(i, vf)| vk::VertexInputBindingDescription {
                binding: i as u32,
                stride: vf.layout.stride,
                input_rate: if vf.instanced {
                    vk::VertexInputRate::INSTANCE
                } else {
                    vk::VertexInputRate::VERTEX
                },
            })
            .collect::<Vec<_>>();
        let vertex_attributes = vs
            .attribute_mappings
            .into_iter()
            .enumerate()
            .map(|(index, mapping)| {
                let (_, ref at) = desc.vertex_fetches[mapping.buffer_index].layout.attributes
                    [mapping.attribute_index];
                vk::VertexInputAttributeDescription {
                    location: index as u32,
                    binding: mapping.buffer_index as u32,
                    format: super::map_vertex_format(at.format),
                    offset: at.offset,
                }
            })
            .collect::<Vec<_>>();

        let vk_vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_buffers)
            .vertex_attribute_descriptions(&vertex_attributes);
        let (raw_topology, supports_restart) = map_primitive_topology(desc.primitive.topology);
        let vk_input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(raw_topology)
            .primitive_restart_enable(supports_restart);

        let mut vk_rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(if desc.primitive.wireframe {
                vk::PolygonMode::LINE
            } else {
                vk::PolygonMode::FILL
            })
            .front_face(map_front_face(desc.primitive.front_face))
            .line_width(1.0);
        if let Some(face) = desc.primitive.cull_mode {
            vk_rasterization = vk_rasterization.cull_mode(map_cull_face(face));
        }

        let mut vk_depth_clip_state =
            vk::PipelineRasterizationDepthClipStateCreateInfoEXT::default()
                .depth_clip_enable(false);
        if desc.primitive.unclipped_depth {
            vk_rasterization = vk_rasterization.push_next(&mut vk_depth_clip_state);
        }

        let dynamic_states = [
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::SCISSOR,
            vk::DynamicState::BLEND_CONSTANTS,
            vk::DynamicState::STENCIL_REFERENCE,
        ];
        let vk_dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let vk_viewport = vk::PipelineViewportStateCreateInfo::default()
            .flags(vk::PipelineViewportStateCreateFlags::empty())
            .scissor_count(1)
            .viewport_count(1);

        let vk_sample_mask = [
            desc.multisample_state.sample_mask as u32,
            (desc.multisample_state.sample_mask >> 32) as u32,
        ];

        let vk_multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::from_raw(
                desc.multisample_state.sample_count,
            ))
            .alpha_to_coverage_enable(desc.multisample_state.alpha_to_coverage)
            .sample_mask(&vk_sample_mask);

        let mut d_format = vk::Format::UNDEFINED;
        let mut s_format = vk::Format::UNDEFINED;
        let mut vk_depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default();
        if let Some(ref ds) = desc.depth_stencil {
            let ds_format = super::map_texture_format(ds.format);
            if ds.format.aspects().contains(crate::TexelAspects::DEPTH) {
                d_format = ds_format;
            }
            if ds.format.aspects().contains(crate::TexelAspects::STENCIL) {
                s_format = ds_format;
            }

            if ds.depth_write_enabled || ds.depth_compare != crate::CompareFunction::Always {
                vk_depth_stencil = vk_depth_stencil
                    .depth_test_enable(true)
                    .depth_write_enable(ds.depth_write_enabled)
                    .depth_compare_op(map_comparison(ds.depth_compare));
            }
            if ds.stencil != crate::StencilState::default() {
                let s = &ds.stencil;
                let front = map_stencil_face_state(&s.front, s.read_mask, s.write_mask);
                let back = map_stencil_face_state(&s.back, s.read_mask, s.write_mask);
                vk_depth_stencil = vk_depth_stencil
                    .stencil_test_enable(true)
                    .front(front)
                    .back(back);
            }

            if ds.bias != crate::DepthBiasState::default() {
                vk_rasterization = vk_rasterization
                    .depth_bias_enable(true)
                    .depth_bias_constant_factor(ds.bias.constant as f32)
                    .depth_bias_clamp(ds.bias.clamp)
                    .depth_bias_slope_factor(ds.bias.slope_scale);
            }
        }

        let mut color_formats = Vec::with_capacity(desc.color_targets.len());
        let mut vk_attachments = Vec::with_capacity(desc.color_targets.len());
        for ct in desc.color_targets {
            let mut vk_attachment = vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::from_raw(ct.write_mask.bits()));
            if let Some(ref blend) = ct.blend {
                let (color_op, color_src, color_dst) = map_blend_component(&blend.color);
                let (alpha_op, alpha_src, alpha_dst) = map_blend_component(&blend.alpha);
                vk_attachment = vk_attachment
                    .blend_enable(true)
                    .color_blend_op(color_op)
                    .src_color_blend_factor(color_src)
                    .dst_color_blend_factor(color_dst)
                    .alpha_blend_op(alpha_op)
                    .src_alpha_blend_factor(alpha_src)
                    .dst_alpha_blend_factor(alpha_dst);
            }

            color_formats.push(super::map_texture_format(ct.format));
            vk_attachments.push(vk_attachment);
        }
        let vk_color_blend =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&vk_attachments);

        let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_formats)
            .depth_attachment_format(d_format)
            .stencil_attachment_format(s_format);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .layout(layout.raw)
            .stages(&stages)
            .vertex_input_state(&vk_vertex_input)
            .input_assembly_state(&vk_input_assembly)
            .rasterization_state(&vk_rasterization)
            .viewport_state(&vk_viewport)
            .multisample_state(&vk_multisample)
            .depth_stencil_state(&vk_depth_stencil)
            .color_blend_state(&vk_color_blend)
            .dynamic_state(&vk_dynamic_state)
            .push_next(&mut rendering_info);

        let mut raw_vec = unsafe {
            self.device
                .core
                .create_graphics_pipelines(self.pipeline_cache.raw, &[create_info], None)
                .unwrap()
        };
        let raw = raw_vec.pop().unwrap();
        self.save_pipeline_cache();

        unsafe { self.device.core.destroy_shader_module(vs.vk_module, None) };
        if let Some(fs) = fs {
            unsafe { self.device.core.destroy_shader_module(fs.vk_module, None) };
        }

        if !desc.name.is_empty() {
            self.set_object_name(raw, desc.name);
        }
        super::RenderPipeline { raw, layout }
    }

    fn destroy_render_pipeline(&self, pipeline: &mut super::RenderPipeline) {
        self.destroy_pipeline_layout(&mut pipeline.layout);
        unsafe {
            self.device.core.destroy_pipeline(pipeline.raw, None);
        }
    }
}

fn map_shader_visibility(visibility: crate::ShaderVisibility) -> vk::ShaderStageFlags {
    use crate::ShaderVisibility as Sv;
    use vk::ShaderStageFlags as Flags;

    let mut flags = Flags::empty();
    if visibility.contains(Sv::COMPUTE) {
        flags |= Flags::COMPUTE;
    }
    if visibility.contains(Sv::VERTEX) {
        flags |= Flags::VERTEX;
    }
    if visibility.contains(Sv::FRAGMENT) {
        flags |= Flags::FRAGMENT;
    }

    flags
}

fn map_primitive_topology(topology: crate::PrimitiveTopology) -> (vk::PrimitiveTopology, bool) {
    use crate::PrimitiveTopology as Pt;
    match topology {
        Pt::PointList => (vk::PrimitiveTopology::POINT_LIST, false),
        Pt::LineList => (vk::PrimitiveTopology::LINE_LIST, false),
        Pt::LineStrip => (vk::PrimitiveTopology::LINE_STRIP, true),
        Pt::TriangleList => (vk::PrimitiveTopology::TRIANGLE_LIST, false),
        Pt::TriangleStrip => (vk::PrimitiveTopology::TRIANGLE_STRIP, true),
    }
}

fn map_front_face(front_face: crate::FrontFace) -> vk::FrontFace {
    match front_face {
        crate::FrontFace::Cw => vk::FrontFace::CLOCKWISE,
        crate::FrontFace::Ccw => vk::FrontFace::COUNTER_CLOCKWISE,
    }
}

fn map_cull_face(face: crate::Face) -> vk::CullModeFlags {
    match face {
        crate::Face::Front => vk::CullModeFlags::FRONT,
        crate::Face::Back => vk::CullModeFlags::BACK,
    }
}

fn map_comparison(fun: crate::CompareFunction) -> vk::CompareOp {
    use crate::CompareFunction as Cf;
    match fun {
        Cf::Never => vk::CompareOp::NEVER,
        Cf::Less => vk::CompareOp::LESS,
        Cf::LessEqual => vk::CompareOp::LESS_OR_EQUAL,
        Cf::Equal => vk::CompareOp::EQUAL,
        Cf::GreaterEqual => vk::CompareOp::GREATER_OR_EQUAL,
        Cf::Greater => vk::CompareOp::GREATER,
        Cf::NotEqual => vk::CompareOp::NOT_EQUAL,
        Cf::Always => vk::CompareOp::ALWAYS,
    }
}

fn map_stencil_op(op: crate::StencilOperation) -> vk::StencilOp {
    use crate::StencilOperation as So;
    match op {
        So::Keep => vk::StencilOp::KEEP,
        So::Zero => vk::StencilOp::ZERO,
        So::Replace => vk::StencilOp::REPLACE,
        So::Invert => vk::StencilOp::INVERT,
        So::IncrementClamp => vk::StencilOp::INCREMENT_AND_CLAMP,
        So::IncrementWrap => vk::StencilOp::INCREMENT_AND_WRAP,
        So::DecrementClamp => vk::StencilOp::DECREMENT_AND_CLAMP,
        So::DecrementWrap => vk::StencilOp::DECREMENT_AND_WRAP,
    }
}

fn map_stencil_face_state(
    face: &crate::StencilFaceState,
    compare_mask: u32,
    write_mask: u32,
) -> vk::StencilOpState {
    vk::StencilOpState {
        fail_op: map_stencil_op(face.fail_op),
        pass_op: map_stencil_op(face.pass_op),
        depth_fail_op: map_stencil_op(face.depth_fail_op),
        compare_op: map_comparison(face.compare),
        compare_mask,
        write_mask,
        reference: 0,
    }
}

fn map_blend_factor(factor: crate::BlendFactor) -> vk::BlendFactor {
    use crate::BlendFactor as Bf;
    match factor {
        Bf::Zero => vk::BlendFactor::ZERO,
        Bf::One => vk::BlendFactor::ONE,
        Bf::Src => vk::BlendFactor::SRC_COLOR,
        Bf::OneMinusSrc => vk::BlendFactor::ONE_MINUS_SRC_COLOR,
        Bf::SrcAlpha => vk::BlendFactor::SRC_ALPHA,
        Bf::OneMinusSrcAlpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        Bf::Dst => vk::BlendFactor::DST_COLOR,
        Bf::OneMinusDst => vk::BlendFactor::ONE_MINUS_DST_COLOR,
        Bf::DstAlpha => vk::BlendFactor::DST_ALPHA,
        Bf::OneMinusDstAlpha => vk::BlendFactor::ONE_MINUS_DST_ALPHA,
        Bf::SrcAlphaSaturated => vk::BlendFactor::SRC_ALPHA_SATURATE,
        Bf::Constant => vk::BlendFactor::CONSTANT_COLOR,
        Bf::OneMinusConstant => vk::BlendFactor::ONE_MINUS_CONSTANT_COLOR,
    }
}

fn map_blend_op(operation: crate::BlendOperation) -> vk::BlendOp {
    use crate::BlendOperation as Bo;
    match operation {
        Bo::Add => vk::BlendOp::ADD,
        Bo::Subtract => vk::BlendOp::SUBTRACT,
        Bo::ReverseSubtract => vk::BlendOp::REVERSE_SUBTRACT,
        Bo::Min => vk::BlendOp::MIN,
        Bo::Max => vk::BlendOp::MAX,
    }
}

fn map_blend_component(
    component: &crate::BlendComponent,
) -> (vk::BlendOp, vk::BlendFactor, vk::BlendFactor) {
    let op = map_blend_op(component.operation);
    let src = map_blend_factor(component.src_factor);
    let dst = map_blend_factor(component.dst_factor);
    (op, src, dst)
}

impl super::Context {
    /// Writes the pipeline cache when it has grown since it was last written:
    /// on the first run (batched every 8 pipelines), not on every pipeline of
    /// a warm launch. (BoxBox patch)
    fn save_pipeline_cache(&self) {
        use std::sync::atomic::Ordering;
        let cache = &self.pipeline_cache;
        let count = cache.creates.fetch_add(1, Ordering::Relaxed) + 1;
        // On a warm run with a valid cache and no new shaders, defer until Drop.
        if cache.initially_usable && !self.shader_cache.is_dirty() {
            return;
        }
        // GPUI creates 8 pipelines per BladePipelines::new; flush once per batch.
        if count % 8 != 0 {
            return;
        }
        self.flush_caches();
    }

    pub(super) fn flush_caches(&self) {
        use std::sync::atomic::Ordering;
        self.shader_cache.save_if_dirty();
        let cache = &self.pipeline_cache;
        let Some(ref file) = cache.file else { return };
        let Ok(data) = (unsafe { self.device.core.get_pipeline_cache_data(cache.raw) }) else { return };
        if data.len() <= cache.saved.load(Ordering::Relaxed) {
            return;
        }
        cache.saved.store(data.len(), Ordering::Relaxed);
        let temporary = file.with_extension(format!("{}.tmp", std::process::id()));
        if std::fs::write(&temporary, &data).and_then(|()| std::fs::rename(&temporary, file)).is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
    }
}

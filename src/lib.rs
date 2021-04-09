#![allow(non_snake_case)]

use std::{
    ffi::{c_void, CString},
    mem::transmute,
};

use ash::{
    version::{EntryV1_0, InstanceV1_0},
    vk::Handle,
};
use libloading::Library;
use log::{error, info};
use openxr_sys::Result as XrResult;

#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "full"))]
pub fn android_main() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::max())
        .try_init();

    let application_name = "test";
    let application_version = 1;
    let engine_name: Option<&str> = None;
    let engine_version: Option<u32> = None;
    let required_layers: &[&str] = &[];
    let required_extensions: &[&str] = &[];

    let entry = XrEntry::load().unwrap();

    info!("xrInitializeLoaderKHR()");
    let (vm, activity) = {
        let initialize_loader_KHR: openxr_sys::pfn::InitializeLoaderKHR = unsafe {
            transmute(
                entry
                    .fp
                    .get_proc_addr(openxr_sys::Instance::NULL, "xrInitializeLoaderKHR"),
            )
        };

        let native_activity = ndk_glue::native_activity();
        let vm = native_activity.vm();
        let activity = native_activity.activity();

        // https://www.khronos.org/registry/OpenXR/specs/1.0/man/html/XrLoaderInitInfoAndroidKHR.html
        let info = Box::into_raw(Box::new(openxr_sys::LoaderInitInfoAndroidKHR {
            ty: openxr_sys::LoaderInitInfoAndroidKHR::TYPE,
            next: std::ptr::null(),
            application_vm: vm as *mut c_void,
            application_context: activity as *mut c_void,
        })) as *const openxr_sys::LoaderInitInfoBaseHeaderKHR;

        let call_result = unsafe { initialize_loader_KHR(info) };

        if call_result != XrResult::SUCCESS {
            panic!("Failed initialize_loader_KHR");
        }

        (vm, activity)
    };

    let application_info = {
        // Prevents application names from being larger than the container in ApplicationInfo
        assert!(
            application_name.len() <= openxr_sys::MAX_APPLICATION_NAME_SIZE,
            "OpenXR application names must be {} bytes or less",
            openxr_sys::MAX_APPLICATION_NAME_SIZE
        );

        // Prevents application names from being empty
        assert!(
            application_name.len() > 0,
            "OpenXR application names must be greater than 0 bytes"
        );

        let mut app_info = openxr_sys::ApplicationInfo {
            application_name: [0; openxr_sys::MAX_APPLICATION_NAME_SIZE],
            engine_name: [0; openxr_sys::MAX_ENGINE_NAME_SIZE],
            application_version,
            engine_version: engine_version.map_or(0, |v| v),
            api_version: openxr_sys::CURRENT_API_VERSION,
        };

        for (app_char, slot) in application_name
            .bytes()
            .zip(app_info.application_name.iter_mut())
        {
            *slot = app_char as _;
        }

        app_info.application_name[application_name.len()] = 0;

        // Its safe to not do anything if `engine_name` is `None` because the
        // buffer is already initialized to 0
        if let Some(name) = engine_name {
            for (engine_char, slot) in name.bytes().zip(app_info.application_name.iter_mut()) {
                *slot = engine_char as _;
            }

            app_info.application_name[application_name.len()] = 0;
        }

        app_info
    };

    // Create NULL-terminated CStrings and collect pointers to them
    // into vectors to be passed to OpenXR.
    let required_layers_cstring = required_layers
        .iter()
        .filter_map(|&name| CString::new(name).ok())
        .collect::<Vec<_>>();

    let required_layer_ptrs = required_layers_cstring
        .iter()
        .map(|layer| layer.as_ptr())
        .collect::<Vec<_>>();

    let additional_extensions = ["XR_KHR_vulkan_enable", "XR_KHR_android_create_instance"];

    let required_exts_cstring = required_extensions
        .iter()
        .chain(additional_extensions.iter())
        .filter_map(|&name| CString::new(name).ok())
        .collect::<Vec<_>>();

    let required_ext_ptrs = required_exts_cstring
        .iter()
        .map(|layer| layer.as_ptr())
        .collect::<Vec<_>>();

    // https://www.khronos.org/registry/OpenXR/specs/1.0/html/xrspec.html#XR_KHR_android_create_instance
    let create_info_ext = Box::into_raw(Box::new(openxr_sys::InstanceCreateInfoAndroidKHR {
        ty: openxr_sys::InstanceCreateInfoAndroidKHR::TYPE,
        next: std::ptr::null(),
        application_vm: vm as *mut c_void,
        application_activity: activity as *mut c_void,
    })) as *const c_void;

    let create_info = openxr_sys::InstanceCreateInfo {
        ty: openxr_sys::InstanceCreateInfo::TYPE,
        next: create_info_ext,
        create_flags: openxr_sys::InstanceCreateFlags::EMPTY,
        application_info,
        enabled_api_layer_count: required_layer_ptrs.len() as _,
        enabled_api_layer_names: required_layer_ptrs.as_ptr(),
        enabled_extension_count: required_ext_ptrs.len() as _,
        enabled_extension_names: required_ext_ptrs.as_ptr(),
    };

    info!("xrCreateInstance()");
    let instance = {
        let mut instance_handle = openxr_sys::Instance::NULL;
        let call_result = unsafe { (entry.fp.create_instance)(&create_info, &mut instance_handle) };
        if call_result != XrResult::SUCCESS {
            panic!("Failed to create_instance");
        }
        instance_handle
    };

    let fp = XrInstanceFp::new(&entry.fp, instance);

    let system_get_info = openxr_sys::SystemGetInfo {
        ty: openxr_sys::SystemGetInfo::TYPE,
        next: std::ptr::null_mut(),
        form_factor: openxr_sys::FormFactor::HEAD_MOUNTED_DISPLAY,
    };

    info!("xrGetSystem()");
    let system_id = {
        let mut system_id = openxr_sys::SystemId::NULL;
        let get_system: openxr_sys::pfn::GetSystem =
            unsafe { transmute(entry.fp.get_proc_addr(instance, "xrGetSystem")) };
        let result = unsafe { get_system(instance, &system_get_info, &mut system_id) };
        if result != XrResult::SUCCESS {
            panic!("Failed xrGetSystem");
        }
        system_id
    };

    info!("xrGetVulkanGraphicsRequirementsKHR()");
    let mut graphics_requirements =
        openxr_sys::GraphicsRequirementsVulkanKHR::out(std::ptr::null_mut());
    let result = unsafe {
        (fp.get_vulkan_graphics_requirements_KHR)(
            instance,
            system_id,
            graphics_requirements.as_mut_ptr(),
        )
    };

    if result != XrResult::SUCCESS {
        panic!("Failed xrGetVulkanGraphicsRequirementsKHR");
    }

    let graphics_requirements = unsafe { graphics_requirements.assume_init() };

    info!(
        "graphics_requirements: min={}, max={}",
        graphics_requirements.min_api_version_supported,
        graphics_requirements.max_api_version_supported,
    );

    let vk_entry = unsafe { ash::Entry::new().unwrap() };

    let extensions = vk_entry
        .enumerate_instance_extension_properties()
        .expect("Failed to get vulkan extensions");

    info!("vulkan extensions: {:#?}", extensions);

    info!("xrGetVulkanInstanceExtensionsKHR()");
    let mut count: u32 = 0;
    let count_ptr: *mut u32 = &mut count;
    let mut buffer = [0; 256];
    let result = unsafe {
        (fp.get_vulkan_instance_extensions_KHR)(
            instance,
            system_id,
            256,
            count_ptr,
            buffer.as_mut_ptr(),
        )
    };
    let req_extensions = &std::str::from_utf8(&buffer).unwrap()[..(count - 1) as usize];
    let req_extensions: Vec<_> = req_extensions
        .split_ascii_whitespace()
        .map(|x| CString::new(x).unwrap())
        .collect();

    if result != XrResult::SUCCESS {
        panic!("Failed xrGetVulkanInstanceExtensionsKHR");
    }

    info!("vulkan ext required: {:?}", req_extensions);

    info!("vkCreateInstance()");
    let vk_instance = {
        let app_name = CString::new("openxr-test").unwrap();
        let engine_name = CString::new("Vulkan Engine").unwrap();
        let app_info = ash::vk::ApplicationInfo {
            s_type: ash::vk::StructureType::APPLICATION_INFO,
            p_next: std::ptr::null(),
            p_application_name: app_name.as_ptr(),
            application_version: 1,
            p_engine_name: engine_name.as_ptr(),
            engine_version: 1,
            api_version: ash::vk::API_VERSION_1_0,
        };

        let extension_names = vec![CString::new("VK_EXT_debug_report").unwrap()];

        let extension_names: Vec<_> = extension_names
            .into_iter()
            .chain(req_extensions.into_iter())
            .collect();

        let extension_names: Vec<_> = extension_names
            .iter()
            .map(|x| x.as_bytes_with_nul().as_ptr())
            .collect();

        let create_info = ash::vk::InstanceCreateInfo {
            s_type: ash::vk::StructureType::INSTANCE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: ash::vk::InstanceCreateFlags::empty(),
            p_application_info: &app_info,
            pp_enabled_layer_names: std::ptr::null(),
            enabled_layer_count: 0,
            pp_enabled_extension_names: extension_names.as_ptr() as *const *const u8,
            enabled_extension_count: extension_names.len() as u32,
        };

        unsafe {
            vk_entry
                .create_instance(&create_info, None)
                .expect("Failed vkCreateInstance()")
        }
    };

    let vk_instance_raw = vk_instance.handle().as_raw() as *const c_void;

    info!("xrGetVulkanGraphicsDeviceKHR()");
    let mut physical_device = std::mem::MaybeUninit::new(std::ptr::null());
    // TODO: Error handling
    let result = unsafe {
        (fp.get_vulkan_graphics_device_KHR)(
            instance,
            system_id,
            vk_instance_raw,
            physical_device.as_mut_ptr(),
        )
    };

    if result != XrResult::SUCCESS {
        panic!("Failed xrGetVulkanGraphicsDeviceKHR");
    }

    let physical_device = unsafe { physical_device.assume_init() };
    let physical_device = ash::vk::PhysicalDevice::from_raw(physical_device as u64);

    let device = std::ptr::null();

    let graphics_binding = openxr_sys::GraphicsBindingVulkanKHR {
        ty: openxr_sys::StructureType::GRAPHICS_BINDING_VULKAN_KHR,
        instance: vk_instance_raw,
        physical_device: physical_device.as_raw() as *const c_void,
        device,
        queue_family_index: 0,
        queue_index: 0,
        next: std::ptr::null_mut(),
    };

    let session_create_info = openxr_sys::SessionCreateInfo {
        ty: openxr_sys::StructureType::SESSION_CREATE_INFO,
        create_flags: openxr_sys::SessionCreateFlags::EMPTY,
        system_id,
        next: Box::into_raw(Box::new(graphics_binding)) as *const c_void,
    };

    info!("xrCreateSession()");
    let mut session = openxr_sys::Session::NULL;
    let result = unsafe { (fp.create_session)(instance, &session_create_info, &mut session) };

    if result != XrResult::SUCCESS {
        panic!("Failed xrCreateSession");
    }
}

struct XrEntry {
    fp: XrEntryFp,
    _lib: Library,
}

impl XrEntry {
    pub fn load() -> Result<Self, libloading::Error> {
        #[cfg(target_os = "windows")]
        const PATH: &str = "openxr_loader.dll";
        #[cfg(target_os = "macos")]
        const PATH: &str = "libopenxr_loader.dylib";
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        const PATH: &str = "libopenxr_loader.so";

        let lib = unsafe { Library::new(PATH)? };
        Ok(unsafe {
            XrEntry {
                fp: XrEntryFp {
                    get_instance_proc_addr: *lib.get(b"xrGetInstanceProcAddr\0")?,
                    create_instance: *lib.get(b"xrCreateInstance\0")?,
                    enumerate_instance_extension_properties: *lib
                        .get(b"xrEnumerateInstanceExtensionProperties\0")?,
                    enumerate_api_layer_properties: *lib.get(b"xrEnumerateApiLayerProperties\0")?,
                },
                _lib: lib,
            }
        })
    }
}

struct XrEntryFp {
    pub get_instance_proc_addr: openxr_sys::pfn::GetInstanceProcAddr,
    pub create_instance: openxr_sys::pfn::CreateInstance,
    pub enumerate_instance_extension_properties:
        openxr_sys::pfn::EnumerateInstanceExtensionProperties,
    pub enumerate_api_layer_properties: openxr_sys::pfn::EnumerateApiLayerProperties,
}

impl XrEntryFp {
    pub unsafe fn get_proc_addr(
        &self,
        instance: openxr_sys::Instance,
        name: &str,
    ) -> Option<openxr_sys::pfn::VoidFunction> {
        let c_name = CString::new(name).unwrap();
        let mut fn_ptr = None;

        let result = (self.get_instance_proc_addr)(instance, c_name.as_ptr(), &mut fn_ptr);

        match result {
            XrResult::SUCCESS => fn_ptr,
            _ => {
                error!("Could not load OpenXR function: {}", name);
                None
            }
        }
    }
}

struct XrInstanceFp {
    get_vulkan_graphics_requirements_KHR: openxr_sys::pfn::GetVulkanGraphicsRequirementsKHR,
    get_vulkan_graphics_device_KHR: openxr_sys::pfn::GetVulkanGraphicsDeviceKHR,
    get_vulkan_instance_extensions_KHR: openxr_sys::pfn::GetVulkanInstanceExtensionsKHR,
    create_session: openxr_sys::pfn::CreateSession,
}

impl XrInstanceFp {
    fn new(fp: &XrEntryFp, instance: openxr_sys::Instance) -> Self {
        unsafe {
            XrInstanceFp {
                get_vulkan_graphics_requirements_KHR: transmute(
                    fp.get_proc_addr(instance, "xrGetVulkanGraphicsRequirementsKHR"),
                ),
                get_vulkan_graphics_device_KHR: transmute(
                    fp.get_proc_addr(instance, "xrGetVulkanGraphicsDeviceKHR"),
                ),
                get_vulkan_instance_extensions_KHR: transmute(
                    fp.get_proc_addr(instance, "xrGetVulkanInstanceExtensionsKHR"),
                ),
                create_session: transmute(fp.get_proc_addr(instance, "xrCreateSession")),
            }
        }
    }
}

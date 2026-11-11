//! Standard OpenFX string constants.

// Actions
pub const OFX_ACTION_LOAD: &str = "OfxActionLoad";
pub const OFX_ACTION_DESCRIBE: &str = "OfxActionDescribe";
pub const OFX_ACTION_UNLOAD: &str = "OfxActionUnload";
pub const OFX_ACTION_CREATE_INSTANCE: &str = "OfxActionCreateInstance";
pub const OFX_ACTION_DESTROY_INSTANCE: &str = "OfxActionDestroyInstance";
pub const OFX_ACTION_PURGE_CACHES: &str = "OfxActionPurgeCaches";
pub const OFX_ACTION_SYNC_PRIVATE_DATA: &str = "OfxActionSyncPrivateData";
pub const OFX_ACTION_INSTANCE_CHANGED: &str = "OfxActionInstanceChanged";
pub const OFX_ACTION_OPENGL_CONTEXT_ATTACHED: &str = "OfxActionOpenGLContextAttached";
// The OFX header spells this one with a leading "k"; plug-ins compare the same string.
pub const OFX_ACTION_OPENGL_CONTEXT_DETACHED: &str = "kOfxActionOpenGLContextDetached";
pub const OFX_IMAGE_EFFECT_ACTION_DESCRIBE_IN_CONTEXT: &str = "OfxImageEffectActionDescribeInContext";
pub const OFX_IMAGE_EFFECT_ACTION_RENDER: &str = "OfxImageEffectActionRender";
pub const OFX_IMAGE_EFFECT_ACTION_BEGIN_SEQUENCE_RENDER: &str = "OfxImageEffectActionBeginSequenceRender";
pub const OFX_IMAGE_EFFECT_ACTION_END_SEQUENCE_RENDER: &str = "OfxImageEffectActionEndSequenceRender";
pub const OFX_IMAGE_EFFECT_ACTION_IS_IDENTITY: &str = "OfxImageEffectActionIsIdentity";
pub const OFX_IMAGE_EFFECT_ACTION_GET_REGION_OF_DEFINITION: &str = "OfxImageEffectActionGetRegionOfDefinition";
pub const OFX_IMAGE_EFFECT_ACTION_GET_REGIONS_OF_INTEREST: &str = "OfxImageEffectActionGetRegionsOfInterest";
pub const OFX_IMAGE_EFFECT_ACTION_GET_TIME_DOMAIN: &str = "OfxImageEffectActionGetTimeDomain";

// Properties
pub const OFX_PROP_TYPE: &str = "OfxPropType";
pub const OFX_PROP_NAME: &str = "OfxPropName";
pub const OFX_PROP_LABEL: &str = "OfxPropLabel";
pub const OFX_PROP_SHORT_LABEL: &str = "OfxPropShortLabel";
pub const OFX_PROP_LONG_LABEL: &str = "OfxPropLongLabel";
pub const OFX_PROP_VERSION: &str = "OfxPropVersion";
pub const OFX_PROP_PLUGIN_DESCRIPTION: &str = "OfxPropPluginDescription";
pub const OFX_PROP_TIME: &str = "OfxPropTime";

// Property Types
pub const OFX_TYPE_IMAGE_EFFECT_HOST: &str = "OfxTypeImageEffectHost";
pub const OFX_TYPE_IMAGE_EFFECT: &str = "OfxTypeImageEffect";
pub const OFX_TYPE_IMAGE_EFFECT_INSTANCE: &str = "OfxTypeImageEffectInstance";
pub const OFX_TYPE_PARAMETER: &str = "OfxTypeParameter";
pub const OFX_TYPE_PARAMETER_INSTANCE: &str = "OfxTypeParameterInstance";
pub const OFX_TYPE_CLIP: &str = "OfxTypeClip";
pub const OFX_TYPE_IMAGE: &str = "OfxTypeImage";

// Image Effect Properties
pub const OFX_IMAGE_EFFECT_PROP_SUPPORTED_CONTEXTS: &str = "OfxImageEffectPropSupportedContexts";
pub const OFX_IMAGE_EFFECT_PROP_SUPPORTED_PIXEL_DEPTHS: &str = "OfxImageEffectPropSupportedPixelDepths";
pub const OFX_IMAGE_EFFECT_PROP_SUPPORTED_COMPONENTS: &str = "OfxImageEffectPropSupportedComponents";
pub const OFX_IMAGE_EFFECT_PROP_CONTEXT: &str = "OfxImageEffectPropContext";
pub const OFX_IMAGE_EFFECT_PROP_RENDER_WINDOW: &str = "OfxImageEffectPropRenderWindow";
pub const OFX_IMAGE_EFFECT_PROP_RENDER_SCALE: &str = "OfxImageEffectPropRenderScale";
pub const OFX_IMAGE_EFFECT_PROP_PRE_MULTIPLICATION: &str = "OfxImageEffectPropPreMultiplication";
pub const OFX_IMAGE_EFFECT_PLUGIN_PROP_GROUPING: &str = "OfxImageEffectPluginPropGrouping";
pub const OFX_IMAGE_EFFECT_PROP_FIELD_TO_RENDER: &str = "OfxImageEffectPropFieldToRender";
pub const OFX_IMAGE_EFFECT_PROP_SEQUENTIAL_RENDER_STATUS: &str = "OfxImageEffectPropSequentialRenderStatus";
pub const OFX_IMAGE_EFFECT_PROP_INTERACTIVE_RENDER_STATUS: &str = "OfxImageEffectPropInteractiveRenderStatus";
pub const OFX_IMAGE_EFFECT_PROP_RENDER_QUALITY_DRAFT: &str = "OfxImageEffectPropRenderQualityDraft";
pub const OFX_IMAGE_EFFECT_PROP_OPENGL_ENABLED: &str = "OfxImageEffectPropOpenGLEnabled";
pub const OFX_IMAGE_EFFECT_PROP_OPENGL_RENDER_SUPPORTED: &str = "OfxImageEffectPropOpenGLRenderSupported";
pub const OFX_IMAGE_EFFECT_PROP_OPENGL_TEXTURE_INDEX: &str = "OfxImageEffectPropOpenGLTextureIndex";
pub const OFX_IMAGE_EFFECT_PROP_OPENGL_TEXTURE_TARGET: &str = "OfxImageEffectPropOpenGLTextureTarget";
pub const OFX_IMAGE_EFFECT_PROP_CPU_RENDER_SUPPORTED: &str = "OfxImageEffectPropCPURenderSupported";
pub const OFX_IMAGE_EFFECT_PROP_OPENCL_ENABLED: &str = "OfxImageEffectPropOpenCLEnabled";
pub const OFX_IMAGE_EFFECT_PROP_CUDA_ENABLED: &str = "OfxImageEffectPropCudaEnabled";
pub const OFX_IMAGE_EFFECT_PROP_CUDA_RENDER_SUPPORTED: &str = "OfxImageEffectPropCudaRenderSupported";
pub const OFX_IMAGE_EFFECT_PROP_CUDA_STREAM: &str = "OfxImageEffectPropCudaStream";
pub const OFX_IMAGE_EFFECT_PROP_CUDA_STREAM_SUPPORTED: &str = "OfxImageEffectPropCudaStreamSupported";
pub const OFX_IMAGE_EFFECT_PROP_METAL_ENABLED: &str = "OfxImageEffectPropMetalEnabled";

// Field values
pub const OFX_IMAGE_FIELD_NONE: &str = "OfxFieldNone";

// Contexts
pub const OFX_IMAGE_EFFECT_CONTEXT_FILTER: &str = "OfxImageEffectContextFilter";
pub const OFX_IMAGE_EFFECT_CONTEXT_GENERAL: &str = "OfxImageEffectContextGeneral";
pub const OFX_IMAGE_EFFECT_CONTEXT_GENERATOR: &str = "OfxImageEffectContextGenerator";
pub const OFX_IMAGE_EFFECT_CONTEXT_TRANSITION: &str = "OfxImageEffectContextTransition";

// Pixel depths & components
pub const OFX_BIT_DEPTH_NONE: &str = "OfxBitDepthNone";
pub const OFX_BIT_DEPTH_BYTE: &str = "OfxBitDepthByte";
pub const OFX_BIT_DEPTH_SHORT: &str = "OfxBitDepthShort";
pub const OFX_BIT_DEPTH_FLOAT: &str = "OfxBitDepthFloat";

pub const OFX_IMAGE_COMPONENT_NONE: &str = "OfxImageComponentNone";
pub const OFX_IMAGE_COMPONENT_RGBA: &str = "OfxImageComponentRGBA";
pub const OFX_IMAGE_COMPONENT_RGB: &str = "OfxImageComponentRGB";
pub const OFX_IMAGE_COMPONENT_ALPHA: &str = "OfxImageComponentAlpha";

pub const OFX_IMAGE_OPAQUE: &str = "OfxImageOpaque";
pub const OFX_IMAGE_PRE_MULTIPLIED: &str = "OfxImagePreMultiplied";
pub const OFX_IMAGE_UN_PRE_MULTIPLIED: &str = "OfxImageUnPreMultiplied";

// Image properties
pub const OFX_IMAGE_PROP_DATA: &str = "OfxImagePropData";
pub const OFX_IMAGE_PROP_BOUNDS: &str = "OfxImagePropBounds";
pub const OFX_IMAGE_PROP_REGION_OF_DEFINITION: &str = "OfxImagePropRegionOfDefinition";
pub const OFX_IMAGE_PROP_ROW_BYTES: &str = "OfxImagePropRowBytes";
pub const OFX_IMAGE_PROP_PIXEL_ASPECT_RATIO: &str = "OfxImagePropPixelAspectRatio";
pub const OFX_IMAGE_PROP_FIELD: &str = "OfxImagePropField";
pub const OFX_IMAGE_PROP_UNIQUE_IDENTIFIER: &str = "OfxImagePropUniqueIdentifier";
pub const OFX_IMAGE_CLIP_PROP_CONNECTED: &str = "OfxImageClipPropConnected";
pub const OFX_IMAGE_CLIP_PROP_OPTIONAL: &str = "OfxImageClipPropOptional";

// Standard Clips
pub const OFX_IMAGE_EFFECT_OUTPUT_CLIP_NAME: &str = "Output";
pub const OFX_IMAGE_EFFECT_SIMPLE_SOURCE_CLIP_NAME: &str = "Source";

// Parameters
pub const OFX_PARAM_TYPE_INTEGER: &str = "OfxParamTypeInteger";
pub const OFX_PARAM_TYPE_DOUBLE: &str = "OfxParamTypeDouble";
pub const OFX_PARAM_TYPE_BOOLEAN: &str = "OfxParamTypeBoolean";
pub const OFX_PARAM_TYPE_CHOICE: &str = "OfxParamTypeChoice";
pub const OFX_PARAM_TYPE_RGBA: &str = "OfxParamTypeRGBA";
pub const OFX_PARAM_TYPE_RGB: &str = "OfxParamTypeRGB";
pub const OFX_PARAM_TYPE_DOUBLE_2D: &str = "OfxParamTypeDouble2D";
pub const OFX_PARAM_TYPE_DOUBLE_3D: &str = "OfxParamTypeDouble3D";
pub const OFX_PARAM_TYPE_INTEGER_2D: &str = "OfxParamTypeInteger2D";
pub const OFX_PARAM_TYPE_INTEGER_3D: &str = "OfxParamTypeInteger3D";
pub const OFX_PARAM_TYPE_STRING: &str = "OfxParamTypeString";
pub const OFX_PARAM_TYPE_GROUP: &str = "OfxParamTypeGroup";
pub const OFX_PARAM_TYPE_PAGE: &str = "OfxParamTypePage";
pub const OFX_PARAM_TYPE_PUSH_BUTTON: &str = "OfxParamTypePushButton";

pub const OFX_PARAM_PROP_DEFAULT: &str = "OfxParamPropDefault";
pub const OFX_PARAM_PROP_MIN: &str = "OfxParamPropMin";
pub const OFX_PARAM_PROP_MAX: &str = "OfxParamPropMax";
pub const OFX_PARAM_PROP_DISPLAY_MIN: &str = "OfxParamPropDisplayMin";
pub const OFX_PARAM_PROP_DISPLAY_MAX: &str = "OfxParamPropDisplayMax";
pub const OFX_PARAM_PROP_DIGITS: &str = "OfxParamPropDigits";
pub const OFX_PARAM_PROP_CHOICE_OPTION: &str = "OfxParamPropChoiceOption";
pub const OFX_PARAM_PROP_SECRET: &str = "OfxParamPropSecret";
pub const OFX_PARAM_PROP_ANIMATES: &str = "OfxParamPropAnimates";

// Suite Names
pub const OFX_PROPERTY_SUITE: &str = "OfxPropertySuite";
pub const OFX_PARAMETER_SUITE: &str = "OfxParameterSuite";
pub const OFX_IMAGE_EFFECT_SUITE: &str = "OfxImageEffectSuite";
pub const OFX_MEMORY_SUITE: &str = "OfxMemorySuite";
pub const OFX_MULTI_THREAD_SUITE: &str = "OfxMultiThreadSuite";
pub const OFX_MESSAGE_SUITE: &str = "OfxMessageSuite";
pub const OFX_PROGRESS_SUITE: &str = "OfxProgressSuite";
pub const OFX_TIMELINE_SUITE: &str = "OfxTimeLineSuite";
pub const OFX_INTERACT_SUITE: &str = "OfxInteractSuite";
pub const OFX_OPENGL_RENDER_SUITE: &str = "OfxImageEffectOpenGLRenderSuite";

// Host and clip capabilities a plug-in may ask about.
pub const OFX_PROP_APIVERSION: &str = "OfxPropAPIVersion";
pub const OFX_PROP_VERSION_LABEL: &str = "OfxPropVersionLabel";
pub const OFX_PROP_IS_INTERACTIVE: &str = "OfxPropIsInteractive";
pub const OFX_PLUGIN_PROP_FILE_PATH: &str = "OfxPluginPropFilePath";
pub const OFX_PROP_INSTANCE_DATA: &str = "OfxPropInstanceData";
pub const OFX_IMAGE_EFFECT_HOST_PROP_IS_BACKGROUND: &str = "OfxImageEffectHostPropIsBackground";
pub const OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_MULTIPLE_CLIP_DEPTHS: &str = "OfxImageEffectHostPropSupportsMultipleClipDepths";
pub const OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_MULTIPLE_CLIP_PARTS: &str = "OfxImageEffectHostPropSupportsMultipleClipPARs";
pub const OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_MULTIPLE_CLIP_REGIONS: &str = "OfxImageEffectHostPropSupportsMultipleClipRegions";
pub const OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_CUSTOM_COMPONENTS: &str = "OfxImageEffectHostPropSupportsCustomComponents";
pub const OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_OPENCL: &str = "OfxImageEffectHostPropSupportsOpenCL";
pub const OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_CUDA: &str = "OfxImageEffectHostPropSupportsCuda";
pub const OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_OVERLAYS: &str = "OfxImageEffectHostPropSupportsOverlays";
pub const OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH: &str = "OfxImageEffectPropPixelDepth";
pub const OFX_IMAGE_EFFECT_PROP_COMPONENTS: &str = "OfxImageEffectPropComponents";
pub const OFX_IMAGE_EFFECT_PROP_FRAME_RANGE: &str = "OfxImageEffectPropFrameRange";
pub const OFX_IMAGE_EFFECT_PROP_FRAME_RATE: &str = "OfxImageEffectPropFrameRate";
pub const OFX_IMAGE_EFFECT_PROP_FRAME_STEP: &str = "OfxImageEffectPropFrameStep";
pub const OFX_IMAGE_EFFECT_PROP_SETTABLE: &str = "OfxImageEffectPropSetable";
pub const OFX_IMAGE_EFFECT_PROP_CAN_IN_PLACE: &str = "OfxImageEffectPropCanInPlace";
pub const OFX_IMAGE_EFFECT_PROP_SUPPORTS_MULTI_RES: &str = "OfxImageEffectPropSupportsMultiResolution";
pub const OFX_IMAGE_EFFECT_PROP_SUPPORTS_TILES: &str = "OfxImageEffectPropSupportsTiles";
pub const OFX_IMAGE_EFFECT_PROP_SUPPORTS_OVERLAYS: &str = "OfxImageEffectPropSupportsOverlays";
pub const OFX_IMAGE_EFFECT_PROP_SUPPORTS_MULTI_RESOLUTION: &str = "OfxImageEffectPropSupportsMultiResolution";
pub const OFX_IMAGE_EFFECT_PROP_TEMPORAL_CLIP_ACCESS: &str = "OfxImageEffectPropTemporalClipAccess";
pub const OFX_IMAGE_EFFECT_PROP_RENDER_THREAD_SAFETY: &str = "OfxImageEffectPropRenderThreadSafety";
pub const OFX_IMAGE_EFFECT_PROP_SEQUENTIAL: &str = "OfxImageEffectInstancePropSequentialRender";
pub const OFX_IMAGE_EFFECT_PROP_IS_IDENTITY: &str = "OfxImageEffectPropIsIdentity";
pub const OFX_PARAM_PROP_PARENT_GROUP: &str = "OfxParamPropParent";
pub const OFX_PARAM_PROP_UI_GROUP: &str = "OfxParamPropGroup";
pub const OFX_PARAM_PROP_TYPE: &str = "OfxParamPropType";
pub const OFX_IMAGE_CLIP_PROP_PIXEL_DEPTH: &str = "OfxImageClipPropPixelDepth";
pub const OFX_IMAGE_CLIP_PROP_COMPONENTS: &str = "OfxImageClipPropComponents";
pub const OFX_IMAGE_CLIP_PROP_SUPPORTED_PIXEL_DEPTHS: &str = "OfxImageClipPropSupportedPixelDepths";
pub const OFX_IMAGE_CLIP_PROP_SUPPORTED_COMPONENTS: &str = "OfxImageClipPropSupportedComponents";
pub const OFX_IMAGE_CLIP_PROP_FRAME_RATE: &str = "OfxImageClipPropFrameRate";
pub const OFX_IMAGE_CLIP_PROP_FRAME_RANGE: &str = "OfxImageClipPropFrameRange";
pub const OFX_IMAGE_CLIP_PROP_IS_MASK: &str = "OfxImageClipPropIsMask";
pub const OFX_IMAGE_CLIP_PROP_FIELD_ORDER: &str = "OfxImageClipPropFieldOrder";

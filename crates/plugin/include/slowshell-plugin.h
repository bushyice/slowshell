/*
 * slowshell native plugin C ABI.
 *
 * This header mirrors crates/plugin/src/lib.rs. See that file for the
 * ownership rules. in short: plugin -> host strings/arrays are borrowed for
 * the duration of the call, host -> plugin handles are valid for the call
 * (except the host API and the ctx ptr, which both live for the plugin).
 */
#ifndef SLOWSHELL_PLUGIN_H
#define SLOWSHELL_PLUGIN_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SL_PLUGIN_ABI_VERSION 1

typedef struct {
  const uint8_t *ptr;
  size_t len;
} SlStr;

typedef struct {
  float r, g, b, a;
} SlColor;

typedef struct {
  float width;
  float radius;
  SlColor color;
} SlBorder;

typedef struct {
  SlColor background;
  bool has_background;
  SlBorder border;
  bool has_border;
  SlColor text_color;
  bool has_text_color;
} SlStyle;

typedef struct {
  SlColor base;
  SlColor crust;
  SlColor mantle;
  SlColor primary;
  SlColor secondary;
  SlColor green;
  SlColor red;
  SlColor blue;
  SlColor yellow;
  SlColor orange;
  SlColor text;
  SlColor subtext;
  SlColor overlay;
} SlTheme;

#define SL_STYLE_VALUE_COLOR 0
#define SL_STYLE_VALUE_INTEGER 1
#define SL_STYLE_VALUE_FLOAT 2
#define SL_STYLE_VALUE_STRING 3
#define SL_STYLE_VALUE_BOOLEAN 4

#define SL_STYLE_COLOR_TRANSPARENT 0
#define SL_STYLE_COLOR_THEME 1
#define SL_STYLE_COLOR_HEX 2

typedef struct {
  SlStr key;
  uint32_t kind;
  uint32_t color_kind;
  SlStr text;
  int32_t integer;
  float float_value;
  bool boolean;
} SlStyleSheetEntry;

typedef struct {
  const SlStyleSheetEntry *entries;
  uint32_t count;
} SlStyleSheet;

#define SL_NODE_ROW 0
#define SL_NODE_COLUMN 1
#define SL_NODE_ICON 2
#define SL_NODE_TEXT 3
#define SL_NODE_PROGRESS 4
#define SL_NODE_CANVAS 5
#define SL_NODE_CONTAINER 6
#define SL_NODE_IMAGE 7
#define SL_NODE_SCROLLABLE 8

#define SL_ALIGN_START 0
#define SL_ALIGN_CENTER 1
#define SL_ALIGN_END 2
#define SL_ALIGN_FILL 3

#define SL_LENGTH_UNIT_SHRINK 0
#define SL_LENGTH_UNIT_FILL 1
#define SL_LENGTH_UNIT_FIXED 2

typedef struct {
  uint32_t unit;
  float value;
} SlLength;

#define SL_IMAGE_RAW_RGBA 0
#define SL_IMAGE_ENCODED 1
#define SL_IMAGE_PATH 2

#define SL_ANIMATION_NONE 0
#define SL_ANIMATION_SLIDE 1

#define SL_ANIMATION_EASING_LINEAR 0
#define SL_ANIMATION_EASING_EASE 1
#define SL_ANIMATION_EASING_EASE_IN 2
#define SL_ANIMATION_EASING_EASE_OUT 3
#define SL_ANIMATION_EASING_EASE_IN_OUT 4

typedef struct {
  uint32_t kind;
  uint32_t easing;
  uint32_t duration_ms;
  float offset_x;
  float offset_y;
  bool tween;
} SlAnimation;

typedef struct {
  uint32_t width;
  uint32_t height;
  uint32_t stride;
  uint8_t *data;
  uint64_t cache_key;
} SlCanvas;

typedef struct {
  uint32_t source;
  SlStr data;
  uint32_t width;
  uint32_t height;
  uint64_t cache_key;
} SlImage;

typedef struct SlNode {
  uint32_t kind;
  SlStyle style;
  float padding[4]; // top, right, bottom, left
  float spacing;
  uint32_t align_x;
  uint32_t align_y;
  const struct SlNode *children;
  size_t child_count;
  SlStr text;
  float size;
  float value;
  SlCanvas canvas;
  SlImage image;
  SlStr action;

  // default is 0
  SlLength width;
  SlLength height;

  bool has_effect;
  size_t effect[4];

  SlAnimation animation;
} SlNode;

typedef struct {
  const SlNode *nodes;
  size_t len;
} SlNodeList;

#define SL_EVENT_NONE 0
#define SL_EVENT_TICK 1
#define SL_EVENT_CONFIG_RELOAD 2
#define SL_EVENT_COMPOSITOR_UPDATE 3
#define SL_EVENT_FD 4
#define SL_EVENT_FRAME 5

#define SL_EVENT_MASK_TICK (1u << 0)
#define SL_EVENT_MASK_CONFIG_RELOAD (1u << 1)
#define SL_EVENT_MASK_COMPOSITOR_UPDATE (1u << 2)
#define SL_EVENT_MASK_FD (1u << 3)
#define SL_EVENT_MASK_FRAME (1u << 4)

#define SL_EFFECT_NONE 0
#define SL_EFFECT_REDRAW 1
#define SL_EFFECT_SUBSCRIBE 2
#define SL_EFFECT_HIDE 3
#define SL_EFFECT_SHOW 4
#define SL_EFFECT_REALLY_HIDE 5
#define SL_EFFECT_DESTROY 6
#define SL_EFFECT_REALLY_DESTROY 7
#define SL_EFFECT_CUSTOM 8

typedef struct {
  uint32_t code;
  size_t custom[4];
} SlEffect;

#define SL_MSG_NOOP 0
#define SL_MSG_EFFECT 1
#define SL_MSG_ACTION 2
#define SL_MSG_EFFECT_ACTION 3

typedef struct {
  uint32_t kind;
  uint32_t effect;
  size_t custom[4];
  SlStr name;
} SlItemMessage;

typedef struct {
  uint32_t kind;
  SlStr name;
  void *bag;
  uint32_t action;
  int32_t fd;
} SlEvent;

typedef struct {
  size_t count;
  const SlStr *keys;
  const SlStr *values;
} SlPayloadArgs;

typedef struct {
  SlStr name;
  uint32_t width;
  uint32_t height;
  float scale;
} SlMonitor;

typedef struct {
  uint64_t id;
  uint8_t idx;
  SlStr output;
  SlStr name;
  bool is_active;
  bool is_focused;
  bool is_urgent;
} SlWorkspace;

typedef struct {
  uint64_t id;
  SlStr title;
  SlStr wclass;
} SlWindow;

typedef struct {
  const SlMonitor *monitors;
  size_t monitor_count;
  const SlWorkspace *workspaces;
  size_t workspace_count;
  const SlWindow *active_window;
  bool overview_active;
} SlCompositorState;



typedef struct {
  SlStr command;
  SlStr label;
} SlNotificationAction;

typedef struct {
  SlStr summary;
  SlStr body;
  SlStr app_name;
  SlStr app_icon;
  uint8_t urgency;
  int32_t timeout_ms;
  uint32_t replaces_id;
  const SlNotificationAction *actions;
  size_t action_count;
} SlNotification;

typedef void *(*SlComponentCreateFn)(void *ctx);
typedef void (*SlComponentDestroyFn)(void *ctx, void *state);
typedef uint32_t (*SlComponentEventsFn)(void *ctx, void *state);
typedef void (*SlComponentWatchFn)(void *ctx, void *state, void *opts);
typedef SlEffect (*SlComponentUpdateFn)(void *ctx, void *state,
                                        const SlEvent *event);
typedef void (*SlComponentViewFn)(void *ctx, void *state, void *opts, SlNodeList *out);
typedef bool (*SlComponentCheckViewFn)(void *ctx, void *state, void *opts);
typedef void (*SlComponentStopFn)(void *ctx, void *state);

typedef struct {
  uint32_t size;
  SlComponentCreateFn create;
  SlComponentDestroyFn destroy;
  SlComponentEventsFn events;
  SlComponentWatchFn watch;
  SlComponentUpdateFn update;
  SlComponentViewFn view;
  SlComponentCheckViewFn check_view;
  SlComponentStopFn stop;
  bool hoverable;
} SlComponentVtable;

typedef void *(*SlCompositorCreateFn)(void *ctx);
typedef void (*SlCompositorDestroyFn)(void *ctx, void *state);
typedef bool (*SlCompositorIsActiveFn)(void *ctx, void *state);
typedef void (*SlCompositorInitializeFn)(void *ctx, void *state);
typedef void (*SlCompositorUpdateFn)(void *ctx, void *state);
typedef void (*SlCompositorCommandFn)(void *ctx, void *state, uint32_t command, int32_t arg);

#define SL_COMPOSITOR_FOCUS_WORKSPACE 0
#define SL_COMPOSITOR_FOCUS_WINDOW 1


#define SL_EPOLL_IN (1u << 0)
#define SL_EPOLL_ET (1u << 1)

typedef struct {
  uint32_t size;
  SlCompositorCreateFn create;
  SlCompositorDestroyFn destroy;
  SlCompositorIsActiveFn is_active;
  SlCompositorInitializeFn initialize;
  SlCompositorUpdateFn update_state;
  SlCompositorCommandFn send_command;
} SlCompositorVtable;



typedef void *(*SlPayloadCreateFn)(void *ctx);
typedef void (*SlPayloadDestroyFn)(void *ctx, void *state);
typedef void (*SlPayloadInvokeFn)(void *ctx, void *state, void *args);

typedef struct {
  uint32_t size;
  SlPayloadCreateFn create;
  SlPayloadDestroyFn destroy;
  SlPayloadInvokeFn invoke;
} SlPayloadVtable;



typedef void *(*SlRenderableCreateFn)(void *ctx);
typedef void (*SlRenderableDestroyFn)(void *ctx, void *state);
typedef void (*SlRenderableViewFn)(void *ctx, void *state, void *opts,
                                   SlNodeList *out);

typedef struct {
  bool wrap_popup;
  float width;
  float height;
} SlRenderableSettings;

typedef void (*SlRenderableSettingsFn)(void *ctx, void *state,
                                       SlRenderableSettings *out);
typedef void (*SlRenderableInitializeFn)(void *ctx, void *state);
typedef SlEffect (*SlRenderableUpdateFn)(void *ctx, void *state);
typedef SlEffect (*SlRenderableHandleMessageFn)(void *ctx, void *state,
                                                const SlItemMessage *message);

typedef struct {
  uint32_t size;
  SlRenderableCreateFn create;
  SlRenderableDestroyFn destroy;
  SlRenderableViewFn view;
  SlRenderableSettingsFn settings;
  SlRenderableInitializeFn initialize;
  SlRenderableUpdateFn update;
  SlRenderableHandleMessageFn handle_message;
} SlRenderableVtable;



typedef struct {
  SlStr icon;
  SlStr title;
  SlStr subtitle;
  SlStr action_label;
  SlStr image;
  const SlStr *tags;
  size_t tag_count;
} SlSpotlightItem;

typedef struct {
  const SlSpotlightItem *items;
  size_t count;
} SlSpotlightList;

typedef void *(*SlSpotlightCreateFn)(void *ctx);
typedef void (*SlSpotlightDestroyFn)(void *ctx, void *state);
typedef int32_t (*SlSpotlightGenerateFn)(void *ctx, void *state, SlStr query,
                                         SlSpotlightList *out);
typedef void (*SlSpotlightActivateFn)(void *ctx, void *state, size_t index);
typedef bool (*SlSpotlightTriggerCheckFn)(void *ctx, void *state, SlStr query);

typedef struct {
  uint32_t size;
  SlSpotlightCreateFn create;
  SlSpotlightDestroyFn destroy;
  SlSpotlightGenerateFn generate;
  SlSpotlightActivateFn activate;
  SlSpotlightTriggerCheckFn trigger_check;
  bool allows_triggers;
  uint32_t display_styles;
} SlSpotlightVtable;

#define SL_DISPLAY_STYLE_LIST 1
#define SL_DISPLAY_STYLE_GRID 2
#define SL_DISPLAY_STYLE_IMAGE_LIST 4



typedef void (*SlRegistryNotifyFn)(void *ctx, size_t key, void *value);
typedef int32_t (*SlRegistryRegisterFn)(void *ctx, size_t key, void *value);
typedef void *(*SlRegistryGetFn)(void *ctx, size_t key);
typedef void *(*SlRegistryRemoveFn)(void *ctx, size_t key);
typedef int32_t (*SlRegistrySubscribeFn)(void *ctx, size_t key,
                                         SlRegistryNotifyFn callback);



typedef struct {
  uint32_t id;
  SlStr name;
  SlStr description;
  float volume;
  bool muted;
  bool is_default;
} SlAudioSink;

typedef struct {
  SlStr identity, title, artist, album, art_url, playback_status;
  bool can_play_pause, can_go_next, can_go_previous;
} SlMprisPlayer;

typedef struct {
  float volume;
  bool muted;
  SlStr default_sink;
  const SlAudioSink *sinks;
  size_t sink_count;
  bool has_player;
  SlMprisPlayer player;
} SlAudioState;

typedef struct {
  SlStr address, name, icon;
  bool paired, connected, trusted;
  int32_t battery;
  int32_t rssi;
} SlBluetoothDevice;

typedef struct {
  bool powered, discovering;
  SlStr adapter_name;
  const SlBluetoothDevice *devices;
  size_t device_count;
} SlBluetoothState;

typedef struct {
  SlStr ssid;
  uint8_t signal;
  bool secured, in_use, saved;
} SlAccessPoint;

typedef struct {
  SlStr iface;
  uint32_t speed;
  bool carrier, connected;
} SlEthernet;

typedef struct {
  SlStr label, mac;
  bool is_wifi;
} SlConnectionInfo;

typedef struct {
  bool wifi_enabled, wifi_hardware_enabled, scanning, busy;
  bool has_wifi_connected;
  SlAccessPoint wifi_connected;
  bool has_ethernet;
  SlEthernet ethernet;
  const SlAccessPoint *networks;
  size_t network_count;
  bool has_connected;
  SlConnectionInfo connected;
} SlNetworkState;

typedef struct {
  uint32_t pid;
  SlStr name;
  float cpu_usage;
  uint64_t memory;
} SlProcessInfo;

typedef struct {
  float cpu_usage, mem_usage;
  uint64_t mem_total, mem_used;
  bool has_temperature;
  float temperature;
  float load[3];
  uint16_t network_state;
  uint64_t network_rx, network_tx;
  const SlProcessInfo *top_processes;
  size_t process_count;
} SlSystemState;

typedef struct {
  SlStr address, title, icon_name, menu_path;
  bool has_pixmap;
  uint32_t pixmap_width, pixmap_height;
  SlStr pixmap;
} SlTrayItem;

typedef struct {
  const SlTrayItem *items;
  size_t item_count;
} SlTrayState;

typedef int32_t (*SlAudioStateGetFn)(void *ctx, SlAudioState *out);
typedef int32_t (*SlBluetoothStateGetFn)(void *ctx, SlBluetoothState *out);
typedef int32_t (*SlNetworkStateGetFn)(void *ctx, SlNetworkState *out);
typedef int32_t (*SlSystemStateGetFn)(void *ctx, SlSystemState *out);
typedef int32_t (*SlTrayStateGetFn)(void *ctx, SlTrayState *out);

typedef struct {
  bool has_percent;
  uint8_t percent;
  bool charging;
  SlStr status;
  bool has_health;
  uint8_t health;
  bool has_energy_now;
  float energy_now_wh;
  bool has_energy_full;
  float energy_full_wh;
  bool has_power;
  float power_w;
  SlStr time_remaining;
  bool has_active_profile;
  SlStr active_profile;
  const SlStr *available_profiles;
  size_t available_profile_count;
  uint8_t brightness_percent;
  uint32_t brightness_max;
  uint32_t brightness_current;
  SlStr device_name;
} SlPowerState;

typedef int32_t (*SlPowerStateGetFn)(void *ctx, SlPowerState *out);
typedef int32_t (*SlDispatchFn)(void *ctx, SlStr command);



#define SL_ANCHOR_TOP (1u << 0)
#define SL_ANCHOR_BOTTOM (1u << 1)
#define SL_ANCHOR_LEFT (1u << 2)
#define SL_ANCHOR_RIGHT (1u << 3)

#define SL_LAYER_BACKGROUND 0
#define SL_LAYER_BOTTOM 1
#define SL_LAYER_TOP 2
#define SL_LAYER_OVERLAY 3

#define SL_KEYBOARD_NONE 0
#define SL_KEYBOARD_ON_DEMAND 1
#define SL_KEYBOARD_EXCLUSIVE 2

#define SL_VISIBILITY_VISIBLE 0
#define SL_VISIBILITY_TRANSIENT 1
#define SL_VISIBILITY_TOGGLEABLE 2

#define SL_UPDATE_ON_DEMAND 0
#define SL_UPDATE_EVERY_FRAME 1
#define SL_UPDATE_ON_EVENT 2

typedef struct {
  uint32_t layer;
  uint32_t anchor;
  uint32_t width;
  uint32_t height;
  int32_t margin[4];
  int32_t exclusive_zone;
  bool events_transparent;
  uint32_t keyboard_interactivity;
  bool per_monitor;
  SlStr monitor;
  SlStr ns;
  uint32_t visibility;
  bool visible;
  uint32_t update_when;
} SlDesktopSettings;

typedef void *(*SlDesktopCreateFn)(void *ctx);
typedef void (*SlDesktopDestroyFn)(void *ctx, void *state);
typedef void (*SlDesktopSettingsFn)(void *ctx, void *state,
                                    SlDesktopSettings *out);
typedef void (*SlDesktopInitializeFn)(void *ctx, void *state, void *opts);
typedef uint32_t (*SlDesktopEventsFn)(void *ctx, void *state);
typedef SlEffect (*SlDesktopUpdateFn)(void *ctx, void *state,
                                      const SlEvent *event);
typedef SlEffect (*SlDesktopHandleMessageFn)(void *ctx, void *state,
                                             const SlItemMessage *message);
typedef void (*SlDesktopViewFn)(void *ctx, void *state, void *opts,
                                SlNodeList *out);

typedef struct {
  uint32_t size;
  SlDesktopCreateFn create;
  SlDesktopDestroyFn destroy;
  SlDesktopSettingsFn settings;
  SlDesktopInitializeFn initialize;
  SlDesktopEventsFn events;
  SlDesktopUpdateFn update;
  SlDesktopViewFn view;
  SlDesktopHandleMessageFn handle_message;
} SlDesktopItemVtable;



typedef void (*SlRegisterComponentFn)(void *host, SlStr name,
                                      const SlComponentVtable *vtable,
                                      void *userdata);
typedef void (*SlRegisterCompositorFn)(void *host, SlStr name,
                                       const SlCompositorVtable *vtable,
                                       void *userdata);
typedef void (*SlRegisterPayloadFn)(void *host, SlStr command,
                                    const SlPayloadVtable *vtable,
                                    void *userdata);
typedef void (*SlRegisterRenderableFn)(void *host, SlStr name,
                                       const SlRenderableVtable *vtable,
                                       void *userdata);
typedef void (*SlRegisterSpotlightFn)(void *host, SlStr name,
                                      const SlSpotlightVtable *vtable,
                                      void *userdata);
typedef void (*SlRegisterDesktopItemFn)(void *host, SlStr name,
                                        const SlDesktopItemVtable *vtable,
                                        void *userdata);
typedef void (*SlRequestRedrawFn)(void *ctx, uint64_t window);
typedef int32_t (*SlRegisterFdFn)(void *ctx, int32_t fd, uint32_t action);
typedef int32_t (*SlSetIntervalFn)(void *ctx, uint32_t millis, bool repeating,
                                   uint32_t tag);
typedef uint8_t *(*SlCanvasAllocFn)(void *ctx, uint32_t width, uint32_t height);
typedef void (*SlLogFn)(void *ctx, int32_t level, SlStr message);
typedef int32_t (*SlGetStrFn)(void *bag, SlStr key, SlStr *out);
typedef int32_t (*SlGetF64Fn)(void *bag, SlStr key, double *out);
typedef int32_t (*SlGetI64Fn)(void *bag, SlStr key, int64_t *out);
typedef int32_t (*SlGetBoolFn)(void *bag, SlStr key, bool *out);
typedef void (*SlSetCompositorStateFn)(void *ctx, const SlCompositorState *state);
typedef int32_t (*SlStyleGetFn)(void *ctx, SlStr name, SlStyleSheet *out);
typedef int32_t (*SlThemeGetFn)(void *ctx, SlTheme *out);
typedef int32_t (*SlStyleRegisterFn)(void *ctx, SlStr name, const SlStyleSheet *sheet);
typedef int32_t (*SlCompositorStateGetFn)(void *ctx, SlCompositorState *out);
typedef int32_t (*SlConfigParserFn)(void *ctx, SlStr name, SlStr block);
typedef int32_t (*SlConfigParserRegisterFn)(void *host, SlStr name,
                                            SlConfigParserFn callback);
typedef void (*SlPluginErrorFn)(void *ctx, uint32_t code, SlStr message);
typedef int32_t (*SlNotifyFn)(void *ctx, const SlNotification *notification);
typedef int32_t (*SlCompositorRegisterFdFn)(void *ctx, int32_t fd, uint32_t flags);
typedef int32_t (*SlPersistenceGetFn)(void *ctx, SlStr key, SlStr *out);
typedef int32_t (*SlPersistenceSetFn)(void *ctx, SlStr key, SlStr value);
typedef int32_t (*SlPersistenceRemoveFn)(void *ctx, SlStr key);
typedef int32_t (*SlDispatchWithStringFn)(void *ctx, SlStr name, SlStr payload);
typedef int32_t (*SlRegisterCommandFn)(void *host, SlStr name, SlStr title,
                                       SlStr description);

typedef struct {
  uint32_t size;
  SlRegisterComponentFn register_component;
  SlRegisterCompositorFn register_compositor;
  SlRequestRedrawFn request_redraw;
  SlRegisterFdFn register_fd;
  SlLogFn log;
  SlGetStrFn get_str;
  SlGetF64Fn get_f64;
  SlGetI64Fn get_i64;
  SlGetBoolFn get_bool;
  SlSetCompositorStateFn set_compositor_state;
  SlSetIntervalFn set_interval;
  SlRegisterPayloadFn register_payload;
  SlRegisterRenderableFn register_renderable;
  SlRegisterDesktopItemFn register_desktop_item;
  SlCanvasAllocFn canvas;
  SlGetStrFn config_get_str;
  SlGetF64Fn config_get_f64;
  SlGetI64Fn config_get_i64;
  SlGetBoolFn config_get_bool;
  SlStyleGetFn style_get;
  SlThemeGetFn theme_get;
  SlStyleRegisterFn style_register;
  SlCompositorStateGetFn compositor_state_get;
  SlConfigParserRegisterFn config_parser_register;
  SlPluginErrorFn plugin_error;
  SlNotifyFn notify;
  SlCompositorRegisterFdFn compositor_register_fd;
  SlRegisterSpotlightFn register_spotlight;
  SlRegistryRegisterFn registry_register;
  SlRegistryGetFn registry_get;
  SlRegistryRemoveFn registry_remove;
  SlRegistrySubscribeFn registry_subscribe;
  SlDispatchFn dispatch;
  SlAudioStateGetFn audio_state_get;
  SlBluetoothStateGetFn bluetooth_state_get;
  SlNetworkStateGetFn network_state_get;
  SlSystemStateGetFn system_state_get;
  SlTrayStateGetFn tray_state_get;
  SlPowerStateGetFn power_state_get;
  SlPersistenceGetFn persistence_get;
  SlPersistenceSetFn persistence_set;
  SlPersistenceRemoveFn persistence_remove;
  SlDispatchWithStringFn dispatch_with_string;
  SlRegisterCommandFn register_command;
} SlHostApi;



typedef struct {
  uint32_t abi_version;
  SlStr id;
  SlStr version;
} SlPluginMeta;

int32_t slowshell_plugin_init(const SlHostApi *api, void *host, void **out_userdata);
void slowshell_plugin_shutdown(void *userdata);
SlPluginMeta slowshell_plugin_meta(void);

#ifdef __cplusplus
}
#endif

#endif

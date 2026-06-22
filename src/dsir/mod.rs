// src/dsir/mod.rs — Pass 7: Device-aware IR Routing
//
// PSIR × DeviceSet → DSIR: 按通路匹配在线设备，降级分配
// DeviceCapability trait + InjectedMode (零设备赛前注入)
//
// 当前骨架在 Anim src/device_map.rs 里——迁移后扩展。

pub mod route;

//! 无窗口交互测试：直接向 egui 注入输入事件，验证悬停展开、切页、按钮。
use crate::app::{App, Env, Options, PH, PW, WIN_H, WIN_W};
#[path="health_reminder_ui_tests.rs"]
mod health_reminder;
#[path="settings_redesign_ui_tests.rs"]
mod settings_redesign;
use eframe::egui::{self, Pos2, Rect, Vec2};
use std::sync::Mutex;

pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(feature = "material")]
#[test]
fn appearance_skin_plugin_preserves_the_builtin_skin_shape_and_dock() {
    use crate::{model::{Dock, IslandShape, Skin}, sdk_spike::local::appearance_assets::Kind};
    use egui::Color32;

    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let mut h = H::new();
    h.app.settings.skin = Skin::Cream;
    h.app.settings.island_shape = IslandShape::Pill;
    h.app.settings.dock = Dock::Bottom;
    let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/examples/celadon-skin/plugin.json"));
    let id = "example.celadon-skin";
    h.app.plugins.install(path).unwrap();
    h.app.plugins.enable(id, true).unwrap();
    h.app.plugins.select_appearance(Kind::Skin, Some(id)).unwrap();
    assert_eq!(h.app.preview_fill(), Color32::from_rgb(238, 245, 236));
    h.app.settings.island_shape = IslandShape::Notch;
    h.frame(Vec::new());
    assert_eq!(h.app.skin_palette().unwrap().ink, [35, 60, 45]);

    h.app.plugins.enable(id, false).unwrap();
    h.frame(Vec::new());
    assert_eq!(h.app.settings.skin, Skin::Cream);
    assert_eq!(h.app.preview_fill(), Color32::from_rgb(255, 243, 228));
    assert_eq!(h.app.settings.island_shape, IslandShape::Notch);
    assert_eq!(h.app.settings.dock, Dock::Bottom);
}

#[cfg(all(windows, feature = "material"))]
#[test]
fn material_foreground_and_shadow_return_to_the_base_skin_when_disabled() {
    use crate::{material::Appearance, model::{IslandShape, Skin}};
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let mut h = H::new();
    h.app.settings.skin = Skin::Classic;
    h.app.settings.island_shape = IslandShape::Pill;
    h.app.settings.pet = false;
    h.app.show_page(1);
    for _ in 0..50 { h.frame(Vec::new()); }

    let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIN_W, WIN_H));
    let color = egui::Color32::from_rgba_unmultiplied(248, 248, 250, 225);
    let mut draw = |material_active, shadow| {
        h.t += 1.0 / 60.0;
        let env = Env {
            t: h.t, material_active,
            material_appearance: Appearance { shadow: Some(shadow), glass_foreground: true, light: true, ..Default::default() },
            ..Default::default()
        };
        h.ctx.run(egui::RawInput { screen_rect: Some(canvas), time: Some(h.t), ..Default::default() }, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx, |ui| {
                let out = h.app.frame(ui, canvas, &env);
                assert_eq!(out.material_layer.is_some(), material_active);
            });
        }).shapes
    };
    let stats = |shapes: &[egui::epaint::ClippedShape]| {
        shapes.iter().filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.rect.width() > 300.0 && rect.rect.height() > 50.0 => Some(rect.fill),
            _ => None,
        }).collect::<Vec<_>>()
    };
    let without_shadow = stats(&draw(true, 0.0));
    let with_shadow = stats(&draw(true, 1.0));
    let disabled = stats(&draw(false, 0.0));
    assert!(without_shadow.contains(&color));
    assert!(with_shadow.len() > without_shadow.len());
    assert!(!disabled.contains(&color));
    assert!(disabled.contains(&egui::Color32::BLACK));
    assert!(disabled.len() > without_shadow.len());
    assert!(!h.app.glassy());
    assert_eq!(h.app.settings.skin, Skin::Classic);
}



#[test]
fn weather_content_tap_switches_both_ways_with_a_short_transition(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for (x,y) in [(100.0,75.0),(270.0,82.0),(314.0,178.0)] {
        let mut h=H::new();h.app.settings.pet=false;
        h.app.weather.snapshot=Some(crate::weather::Snapshot{place:"杭州".into(),temp:19.0,sunrise:Some("06:52".into()),sunset:Some("17:21".into()),..Default::default()});
        h.app.show_page(8);let p=H::local(x,y);h.hover(p,1.0);h.click(p);
        assert!(h.app.weather.solar_open,"天气内容区 ({x},{y}) 应可以轻点切换");
        h.hover(p,0.05);
        assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("返回天气")&&t.fallback_color.a()>0&&t.fallback_color.a()<255)),"切换时应有短暂淡入动效");
        h.hover(p,0.3);h.click(p);assert!(!h.app.weather.solar_open,"日照内容区也可以轻点返回");
        h.hover(p,0.3);
        assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()=="19°"&&t.fallback_color.a()==255)),"动效完成后应恢复清晰的天气概览");
    }
}


#[test]
fn weather_week_shows_all_seven_days(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.weather.snapshot=Some(crate::weather::Snapshot{place:"杭州".into(),temp:19.0,
        days:["今天","明天","周五","周六","周日","周一","周二"].into_iter().map(|label|crate::weather::Day{label:label.into(),high:23.0,low:15.0,..Default::default()}).collect(),
        at:chrono::Utc::now().timestamp(),..Default::default()});
    h.app.show_page(8);h.hover(H::local(250.0,65.0),1.0);
    for label in ["今天","明天","周五","周六","周日","周一","周二"] {
        assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()==label)),"七天预报缺少 {label}");
    }
}

#[test]
fn weather_week_keeps_each_forecast_date_visible(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.weather.snapshot=Some(crate::weather::parse("杭州",&serde_json::json!({"current":{"temperature_2m":19},
        "daily":{"time":["2026-11-28","2026-11-29","2026-11-30","2026-12-01","2026-12-02","2026-12-03","2026-12-04"]}})).unwrap());
    h.app.show_page(8);h.hover(H::local(250.0,65.0),1.0);
    for date in ["11/28","11/29","11/30","12/01","12/02","12/03","12/04"] {
        assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()==date)),"七天预报缺少日期 {date}");
    }
    for s in &h.shapes {if let egui::Shape::Text(t)=&s.shape {assert!(s.clip_rect.contains_rect(t.visual_bounding_rect()),"日期与预报文字不应裁切：{}",t.galley.text());}}
}

#[test]
fn weather_today_opens_solar_details_and_returns_to_forecast(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.weather.snapshot=Some(crate::weather::Snapshot{place:"杭州".into(),temp:19.0,sunrise:Some("06:52".into()),sunset:Some("17:21".into()),sun_times:None,..Default::default()});
    h.app.show_page(8);h.hover(H::local(250.0,65.0),1.0);h.click(H::local(100.0,75.0));h.hover(H::local(100.0,75.0),0.3);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("返回天气"))),"点击当天区域后应进入日出日落视图");
    for phrase in ["日出","日落","06:52","17:21","日照进度待更新"] {
        assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains(phrase))),"旧缓存日照视图缺少 {phrase}");
    }
    assert!(!h.app.weather.loading,"浏览日照视图不应额外发起网络请求");
    h.click(H::local(80.0,44.0));
    assert!(!h.app.weather.solar_open);
    h.hover(H::local(80.0,44.0),0.3);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()=="日出 06:52")),"返回后应显示天气视图的日照入口");
}

#[test]
fn weather_side_bar_shows_a_clickable_temperature_on_both_edges(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for dock in [crate::model::Dock::Left,crate::model::Dock::Right] {
        let mut h=H::new();h.app.settings.dock=dock;
        h.app.weather.snapshot=Some(crate::weather::Snapshot{place:"杭州".into(),temp:-12.0,code:0,day:true,..Default::default()});
        for _ in 0..60{side_frame(&mut h,Some(Pos2::new(250.0,580.0)));}
        let response=h.ctx.read_response(egui::Id::new(("card",true,"weather-side"))).expect("两侧都应提供常驻天气入口");
        let temp=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()=="12°"=>Some((s.clip_rect,t.visual_bounding_rect())),_=>None}).expect("负温度应完整显示");
        assert!(temp.0.contains_rect(temp.1),"{dock:?} 负温度不应裁切");
        let click=response.rect.center();
        side_frame(&mut h,Some(click));
        for pressed in [true,false] {
            h.t+=1.0/60.0;
            let env=Env{t:h.t,clock:"09:41:00".into(),cursor:Some(click),..Default::default()};
            let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(500.0,600.0))),time:Some(h.t),focused:true,
                events:vec![egui::Event::PointerButton{pos:click,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}],..Default::default()};
            let _=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{h.app.frame(ui,ui.max_rect(),&env);});});
        }
        assert_eq!(h.app.page(),8,"{dock:?} 点击侧边温度应进入天气页");
        assert!(!h.app.weather.solar_open);
        h.app.settings.visible_pages[8]=false;
        assert!(!h.app.side_items().contains(&crate::side_dock::SideItem::Weather));
        h.app.settings.visible_pages[8]=true;h.app.settings.minimal=true;
        assert!(!h.app.side_items().contains(&crate::side_dock::SideItem::Weather));
    }
}

#[test]
fn weather_solar_view_preserves_readable_times_across_skins_and_scales(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    use chrono::TimeZone;
    let now=chrono_tz::Asia::Shanghai.with_ymd_and_hms(2026,10,28,9,41,0).single().unwrap().with_timezone(&chrono::Local);
    for skin in crate::model::Skin::ALL {for scale in [1.0,1.25,1.5] {
        let mut h=H::new();h.app.settings.skin=skin;h.ctx.set_pixels_per_point(scale);
        h.app.weather.snapshot=Some(crate::weather::parse("杭州",&serde_json::json!({"timezone":"Asia/Shanghai","current":{"temperature_2m":19},
            "daily":{"time":["2026-10-28"],"sunrise":["2026-10-28T06:52"],"sunset":["2026-10-28T17:21"]}})).unwrap());
        h.app.weather.solar_open=true;h.app.show_page(8);
        for _ in 0..60 {
            h.t+=1.0/60.0;
            let cursor=H::local(180.0,185.0);
            let env=Env{t:h.t,clock:now.format("%H:%M:%S").to_string(),today:now.format("%Y-%m-%d").to_string(),date:now.format("%m月%d日").to_string(),cursor:Some(cursor),..Default::default()};
            let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(crate::app::CANVAS_W,WIN_H))),time:Some(h.t),focused:true,
                events:vec![egui::Event::PointerMoved(cursor)],..Default::default()};
            h.shapes=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{
                h.app.frame(ui,Rect::from_min_size(ui.max_rect().min,Vec2::new(WIN_W,WIN_H)),&env);
            });}).shapes;
        }
        for phrase in ["返回天气","距离日落","07:40","白昼中点","12:06","06:52","17:21","白昼 10小时29分"] {
            assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains(phrase))),"{skin:?} / {scale} 日照视图缺少 {phrase}");
        }
        for s in &h.shapes {if let egui::Shape::Text(t)=&s.shape {assert!(s.clip_rect.contains_rect(t.visual_bounding_rect()),"日照文字不应裁切：{}",t.galley.text());}}
    }}
}

#[test]
#[ignore="按需导出七日预报、日照和两侧常驻天气的实际画面"]
fn weather_foundation_preview(){
    use crate::model::{Dock,Skin};
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_WEATHER_PREVIEW_DIR").expect("需指定预览目录"));std::fs::create_dir_all(&dest).unwrap();
    for (name,dock,solar,clock,skin) in [
        ("weather-week",Dock::Top,false,"14:08:00",Skin::ALL[0]),
        ("weather-sun",Dock::Top,true,"14:08:00",Skin::ALL[0]),
        ("weather-night",Dock::Top,true,"20:08:00",Skin::ALL[0]),
        ("weather-side-left",Dock::Left,false,"14:08:00",Skin::ALL[0]),
        ("weather-side-right",Dock::Right,false,"14:08:00",Skin::ALL[0]),
        ("weather-week-cream",Dock::Top,false,"14:08:00",Skin::Cream),
        ("weather-sun-cream",Dock::Top,true,"14:08:00",Skin::Cream),
        ("weather-week-glass",Dock::Top,false,"14:08:00",Skin::GlassDark),
        ("weather-sun-glass",Dock::Top,true,"14:08:00",Skin::GlassDark),
        ("weather-city-picker",Dock::Top,false,"14:08:00",Skin::Classic),
    ] {
        let mut h=H::new();h.app.settings.dock=dock;h.app.settings.skin=skin;h.app.settings.pet=false;
        let mut snapshot=crate::weather::parse("杭州",&serde_json::json!({"timezone":"Asia/Shanghai",
            "current":{"temperature_2m":19,"apparent_temperature":18,"relative_humidity_2m":58,"wind_speed_10m":12,"weather_code":2,"is_day":1},
            "daily":{"time":["2026-11-11","2026-11-12","2026-11-13","2026-11-14","2026-11-15","2026-11-16","2026-11-17"],
                "weather_code":[2,0,3,61,80,2,0],"temperature_2m_max":[23,24,22,19,18,20,22],"temperature_2m_min":[15,16,14,13,12,12,14],
                "precipitation_probability_max":[10,0,20,70,55,10,0],"sunrise":["2026-11-11T06:52"],"sunset":["2026-11-11T17:21"]}})).unwrap();
        if clock=="20:08:00" {snapshot.day=false;}
        h.app.weather.snapshot=Some(snapshot);
        if name=="weather-city-picker" {
            let city=|label:&str,region:&str,lat:f64|crate::weather::Position{lat,lon:120.0,label:label.into(),region:region.into()};
            h.app.settings.weather_saved=vec![city("杭州","浙江 · 中国",30.27),city("上海","上海 · 中国",31.23),city("哈尔滨","黑龙江 · 中国",45.75)];
            h.app.settings.weather_city="杭州".into();h.app.settings.weather_position=Some(h.app.settings.weather_saved[0].clone());
            h.app.weather.snapshot.as_mut().unwrap().position=h.app.settings.weather_position.clone();
            h.app.weather.picker_open=true;
        }
        if dock==Dock::Top {h.app.weather.solar_open=solar;h.app.show_page(8);}
        let (w,hgt)=if dock==Dock::Top {(crate::app::CANVAS_W,WIN_H)}else{(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H)};
        let mut raster=feature_raster_sized(w,hgt);
        let frames=90;
        for frame in 0..frames {
            h.t+=1.0/60.0;
            let cursor=Some(if dock==Dock::Top{H::local(180.0,185.0)}else{Pos2::new(250.0,580.0)});
            let env=Env{t:h.t,clock:clock.into(),today:"2026-11-11".into(),date:"11月11日 周三".into(),cursor,battery:Some(76),..Default::default()};
            let events=vec![egui::Event::PointerMoved(cursor.unwrap())];
            let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,
                events,viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(2.0),..Default::default()})).collect(),..Default::default()};
            let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{
                h.app.frame(ui,Rect::from_min_size(ui.max_rect().min,Vec2::new(if dock==Dock::Top{WIN_W}else{w},hgt)),&env);
            });});
            raster.update_textures(&output.textures_delta);
            if frame==frames-1 {
                raster.buf.fill([0.20,0.24,0.32,1.0]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
                let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
                let image=image::RgbaImage::from_raw((w*2.0)as u32,(hgt*2.0)as u32,rgba).unwrap();
                let (x,y,crop_w,crop_h)=if dock==Dock::Top {(200,0,880,(h.app.expanded_height()*2.0+24.0)as u32)}else{
                    let rect=h.app.island_rect(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt)),h.app.w.pos,h.app.h.pos);
                    (if dock==Dock::Left{0}else{880},((rect.top()-12.0)*2.0)as u32,120,((rect.height()+24.0)*2.0)as u32)
                };
                let file=format!("{name}.png");
                image::imageops::crop_imm(&image,x,y,crop_w,crop_h).to_image().save(dest.join(file)).unwrap();
            }
            raster.free_textures(&output.textures_delta);
        }
    }
}

/// #200：维语、藏语歌曲在收起的岛上歌词和音乐页里的显示，导出修复前后对照图。
/// 运行：设置 LINGQI_SCRIPT_PREVIEW_DIR 后执行 `cargo test complex_script_lyrics_preview -- --ignored`。

#[test]
#[ignore="按需导出进入日照视图时的涟漪与太阳滑入过程"]
fn weather_solar_entry_preview(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_WEATHER_PREVIEW_DIR").expect("需指定预览目录"));std::fs::create_dir_all(&dest).unwrap();
    let mut h=H::new();h.app.settings.pet=false;
    h.app.weather.snapshot=Some(crate::weather::parse("杭州",&serde_json::json!({"timezone":"Asia/Shanghai",
        "current":{"temperature_2m":19,"apparent_temperature":18,"relative_humidity_2m":58,"wind_speed_10m":12,"weather_code":2,"is_day":1},
        "daily":{"time":["2026-11-11"],"weather_code":[2],"temperature_2m_max":[23],"temperature_2m_min":[15],
            "precipitation_probability_max":[10],"sunrise":["2026-11-11T06:52"],"sunset":["2026-11-11T17:21"]}})).unwrap());
    h.app.show_page(8);
    let (w,hgt)=(crate::app::CANVAS_W,WIN_H);
    let mut raster=feature_raster_sized(w,hgt);
    let click=H::local(330.0,94.0);
    // 第 60 帧按下、61 帧松开；之后按时间点导出。
    let shots=[(70,"weather-entry-a"),(85,"weather-entry-b"),(110,"weather-entry-c"),(160,"weather-entry-d")];
    for frame in 0..161 {
        h.t+=1.0/60.0;
        let env=Env{t:h.t,clock:"14:08:00".into(),today:"2026-11-11".into(),date:"11月11日 周三".into(),cursor:Some(click),battery:Some(76),..Default::default()};
        let mut events=vec![egui::Event::PointerMoved(click)];
        if frame==60||frame==61 {events.push(egui::Event::PointerButton{pos:click,button:egui::PointerButton::Primary,pressed:frame==60,modifiers:Default::default()});}
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,events,
            viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(2.0),..Default::default()})).collect(),..Default::default()};
        let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{
            h.app.frame(ui,Rect::from_min_size(ui.max_rect().min,Vec2::new(WIN_W,hgt)),&env);
        });});
        raster.update_textures(&output.textures_delta);
        if let Some((_,name))=shots.iter().find(|(at,_)|*at==frame) {
            raster.buf.fill([0.20,0.24,0.32,1.0]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
            let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
            let image=image::RgbaImage::from_raw((w*2.0)as u32,(hgt*2.0)as u32,rgba).unwrap();
            image::imageops::crop_imm(&image,200,0,880,(h.app.expanded_height()*2.0+24.0)as u32).to_image().save(dest.join(format!("{name}.png"))).unwrap();
        }
        raster.free_textures(&output.textures_delta);
    }
    assert!(h.app.weather.solar_open,"点击天气视图后应进入日照视图");
}

#[test]
fn island_meshes_stay_valid_after_long_uptime_and_repeated_hover(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();h.app.page=0;h.power=(Some(100),true);
    let mut frames=0;
    for days in [0.0,1.0,3.0,7.0,30.0] {
        h.t=days*86400.0;
        for skin in crate::model::Skin::ALL {
            h.app.settings.skin=skin;
            for scale in [1.0,1.25,1.5] {
                h.ctx.set_pixels_per_point(scale);
                for frame in 0..120 {
                    let point=if frame%60<30{H::local(200.0,15.0)}else{Pos2::new(900.0,250.0)};
                    h.cursor=Some(point);h.frame(vec![egui::Event::PointerMoved(point)]);
                    let meshes=h.ctx.tessellate(h.shapes.clone(),h.ctx.pixels_per_point());
                    for primitive in meshes {if let egui::epaint::Primitive::Mesh(mesh)=primitive.primitive {
                        assert!(mesh.is_valid(),"运行 {days} 天后的网格索引必须有效");
                        assert!(mesh.vertices.iter().all(|v|v.pos.is_finite()&&v.uv.is_finite()),"运行 {days} 天后展开不能出现非有限坐标");
                        assert!(mesh.vertices.iter().all(|v|v.pos.x.abs()<10000.0&&v.pos.y.abs()<10000.0),"运行 {days} 天后顶点不能拉到窗口远处");
                    }}
                    frames+=1;
                }
            }
        }
    }
    eprintln!("长运行绘制检查：{frames} 帧；覆盖 0、1、3、7、30 天及全部皮肤、三种缩放。");
}


#[test]
fn weather_hidden_page_opens_its_settings_without_panicking(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.set_page_visible(8,false);h.app.show_page(8);
    assert!(h.app.settings_open);assert_eq!(h.app.settings_tab,12);
}

#[test]
fn weather_settings_maps_to_the_module_switch(){
    assert_eq!(crate::extras::settings_tab_module(12),Some(8));
}

#[test]
fn weather_city_draft_keeps_the_saved_location_until_confirmed(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.settings.weather_city="杭州".into();h.app.settings.wecom_notifications=true;
    h.app.settings.feishu_notifications=true;h.app.settings.dingtalk_notifications=true;
    h.app.settings.save();h.app.weather.city_input="北京".into();
    h.frame(vec![]);
    assert_eq!(h.app.settings.weather_city,"杭州");
    let saved=crate::model::Settings::load();
    assert_eq!(saved.weather_city,"杭州");
    assert!(saved.wecom_notifications && saved.feishu_notifications && saved.dingtalk_notifications);
    h.app.set_page_visible(8,false);h.app.add_weather_city();
    assert_eq!(h.app.settings.weather_city,"杭州");assert!(!h.app.weather.loading);
}

#[test]
fn weather_cities_switch_from_the_island_title_and_keep_their_own_cache(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.settings.pet=false;
    let hz=crate::weather::Position{lat:30.27,lon:120.15,label:"杭州".into(),region:"浙江 · 中国".into()};
    let bj=crate::weather::Position{lat:39.90,lon:116.40,label:"北京".into(),region:"北京 · 中国".into()};
    // 设置页添加两个城市：只加入列表，搜索框清空。
    for city in [&hz,&bj] {
        h.app.weather.city_input=city.label.clone();h.app.weather.selected_city=Some(city.clone());
        assert!(h.app.add_weather_city());
        assert!(h.app.settings.weather_city.is_empty(),"添加城市只加入列表，不切换当前城市");
        assert!(h.app.weather.city_input.is_empty());
    }
    assert_eq!(h.app.settings.weather_saved,vec![hz.clone(),bj.clone()]);
    h.app.weather.city_input=hz.label.clone();h.app.weather.selected_city=Some(hz.clone());
    assert!(!h.app.add_weather_city(),"重复城市不再添加");
    assert_eq!(crate::model::Settings::load().weather_saved,vec![hz.clone(),bj.clone()],"城市列表应保存到设置");
    // 打开天气页，点标题展开下拉框，选「杭州」。
    h.app.weather.snapshot=Some(crate::weather::Snapshot{place:"北京".into(),position:Some(bj.clone()),temp:-3.0,code:71,day:true,at:chrono::Utc::now().timestamp(),..Default::default()});
    h.app.show_page(8);h.hover(H::local(250.0,65.0),1.0);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()=="当地")),"标题保持「当地天气」");
    h.click(H::local(115.0,112.0));h.hover(H::local(330.0,200.0),0.4);
    assert!(h.app.weather.picker_open,"点地点行展开城市下拉");assert!(!h.app.weather.solar_open,"点地点行不应切到日照");
    for phrase in ["自动定位","杭州","北京","添加或管理城市"] {
        assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains(phrase))),"下拉框缺少 {phrase}");
    }
    let row=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()=="杭州"=>Some(t.visual_bounding_rect().center()),_=>None}).unwrap();
    h.click(row);h.hover(row,0.3);
    assert_eq!(h.app.settings.weather_city,"杭州");assert!(!h.app.weather.picker_open);assert!(!h.app.weather.solar_open);
    assert!(h.app.weather.snapshot.is_none(),"杭州没有缓存时不能显示北京的天气");
    // 点面板外收起，不触发日照视图。
    h.click(H::local(209.0,147.0));h.hover(H::local(330.0,200.0),0.4);assert!(h.app.weather.picker_open,"无数据时入口在页面中下方");
    h.click(H::local(330.0,200.0));h.hover(H::local(330.0,200.0),0.3);
    assert!(!h.app.weather.picker_open);assert!(!h.app.weather.solar_open,"点外面只收起下拉框");
    // 删除当前城市：回到自动定位。
    h.app.remove_weather_city(&hz);
    assert_eq!(h.app.settings.weather_saved,vec![bj.clone()]);assert!(h.app.settings.weather_city.is_empty());assert!(h.app.settings.weather_position.is_none());
}

#[test]
fn weather_negative_temperature_draws_a_hyphen_length_minus_on_the_digit_midline(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.settings.pet=false;
    h.app.weather.snapshot=Some(crate::weather::Snapshot{place:"哈尔滨".into(),temp:-6.0,code:71,day:true,at:chrono::Utc::now().timestamp(),..Default::default()});
    for _ in 0..60{h.frame(vec![]);}
    let digits=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()=="6°"=>Some(t.visual_bounding_rect()),_=>None}).expect("胶囊应显示 6°");
    let bar=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Rect(r) if (digits.left()-r.rect.right()).abs()<4.0 && r.rect.right()<digits.center().x && r.rect.width()>r.rect.height()*1.5 && r.rect.height()<4.0=>Some(r.rect),_=>None}).expect("6° 左侧应有手绘减号");
    assert!(!h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains('−'))),"不再使用过长的排版减号");
    assert!(bar.width()<digits.height()*0.6,"减号长度与连字符相当");
    assert!((bar.center().y-digits.center().y).abs()<digits.height()*0.2,"减号在数字中线上");
}

#[test]
fn weather_city_ime_enter_keeps_focus_and_waits_for_confirmed_text(){
    let _guard=ENV_LOCK.lock().unwrap_or_else(|err|err.into_inner());let mut harness=H::new();
    let mut draw=|events| {
        harness.t+=1.0;harness.app.t=harness.t;
        let input=egui::RawInput{events,time:Some(harness.t),screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(560.0,380.0))),..Default::default()};
        let output=harness.ctx.run(input,|ctx|{egui::CentralPanel::default().show(ctx,|ui|{assert!(!harness.app.weather_city_picker(ui));});});
        let hint=output.shapes.iter().find_map(|shape|match &shape.shape{egui::Shape::Text(text) if text.galley.text()=="输入城市名称搜索"=>Some(text.visual_bounding_rect().center()),_=>None});
        (hint,harness.ctx.memory(|memory|memory.focused()),harness.app.weather.city_input.clone())
    };
    let point=draw(vec![]).0.unwrap();
    for pressed in [true,false]{draw(vec![egui::Event::PointerMoved(point),egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}]);}
    let focused=draw(vec![egui::Event::Ime(egui::ImeEvent::Enabled),egui::Event::Ime(egui::ImeEvent::Preedit("hang".into()))]).1;
    assert!(focused.is_some());
    let enter=||egui::Event::Key{key:egui::Key::Enter,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()};
    let (_,after,text)=draw(vec![enter(),egui::Event::Ime(egui::ImeEvent::Commit("杭州".into()))]);
    assert_eq!(focused,after);assert_eq!(text,"杭州");
    assert_ne!(draw(vec![enter()]).1,focused);
}

#[test]
fn weather_city_dropdown_selects_the_region_without_applying_the_draft(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let correct=crate::weather::Position{lat:23.02677,lon:113.13148,label:"佛山市".into(),region:"广东 · 中国".into()};
    let wrong=crate::weather::Position{lat:28.73401,lon:98.80612,label:"佛山".into(),region:"迪庆藏族自治州 · 云南 · 中国".into()};
    h.app.settings.weather_city="杭州".into();h.app.weather.manual_city=true;h.app.weather.city_input="佛山".into();
    h.app.weather.cities.seed("佛山",vec![correct.clone(),wrong]);
    fn draw(h:&mut H,events:Vec<egui::Event>){
        h.t+=1.0/60.0;h.app.t=h.t;
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(560.0,380.0))),time:Some(h.t),events,..Default::default()};
        h.shapes=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().show(ctx,|ui|{h.app.weather_city_picker(ui);});}).shapes;
    }
    for _ in 0..3 {draw(&mut h,vec![]);}
    h.ctx.memory_mut(|m|m.open_popup(egui::Id::new("weather-city-options")));
    for _ in 0..3 {draw(&mut h,vec![]);}
    let point=h.shapes.iter().find_map(|s|match &s.shape {egui::Shape::Text(text) if text.galley.text()=="佛山市"=>Some(text.visual_bounding_rect().center()),_=>None}).expect("下拉框必须显示完整城市名");
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()=="广东 · 中国")),"下拉框必须显示省份");
    draw(&mut h,vec![egui::Event::PointerMoved(point),egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
    draw(&mut h,vec![egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
    assert_eq!(h.app.weather.selected_city.as_ref(),Some(&correct));assert_eq!(h.app.weather.draft_city(),Some("佛山市"));
    assert_eq!(h.app.settings.weather_city,"杭州","选择候选后仍需应用确认");assert!(!h.app.weather.loading);
    assert!(!h.ctx.memory(|m|m.is_popup_open(egui::Id::new("weather-city-options"))));
    h.app.settings.weather_city="佛山市".into();h.app.settings.weather_position=Some(correct.clone());h.app.settings.save();
    let saved=crate::model::Settings::load();assert_eq!(saved.weather_position,Some(correct));
    h.app.weather.city_input="伦敦".into();assert_eq!(h.app.weather.draft_city(),None,"改写输入后必须重新选择地区");
}

#[test]
fn weather_attribution_opens_its_source_without_changing_pages(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(8);h.hover(H::local(300.0,60.0),1.0);
    h.click(H::local(375.0,crate::app::WEATHER_H-7.0));
    assert!(h.cmds.contains(&crate::app::Cmd::OpenUrl("https://open-meteo.com/".into())));
    assert_eq!(h.app.page(),8);
}


#[test]
fn resized_font_atlases_stop_reuploading_and_release_unused_sizes(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let ctx=egui::Context::default();crate::theme::load_fonts(&ctx);
    ctx.set_embed_viewports(false);
    let child=egui::ViewportId::from_hash_of("font-upload-settings");
    let children=std::sync::Arc::new(Mutex::new(Vec::new()));let capture=children.clone();
    egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
        let id=viewport.ids.this;
        let input=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(640.0,270.0))),max_texture_side:Some(2048),
            viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()};
        capture.lock().unwrap().push(ctx.run(input,|ctx|(viewport.viewport_ui_cb)(ctx)));
    });
    let frame=|ppp:f32,settings:bool|{
        let mut viewports=std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(ppp),..Default::default()})).collect::<egui::ViewportIdMap<_>>();
        if settings{viewports.insert(child,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()});}
        let input=egui::RawInput{viewports,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(640.0,270.0))),max_texture_side:Some(2048),..Default::default()};
        let draw=|ctx:&egui::Context|{egui::CentralPanel::default().show(ctx,|ui|{ui.label("专注时间 25:00 · 休息 · 音乐与歌词");});};
        let root=ctx.run(input,|ctx|{draw(ctx);if settings{ctx.show_viewport_immediate(child,Default::default(),|ctx,_|draw(ctx));}});
        let mut outputs=std::mem::take(&mut *children.lock().unwrap());outputs.push(root);
        outputs
    };
    for settings in [false,true,false] {
        for ppp in [1.0,1.4,1.2,0.8,1.4,1.0] {
            for _ in 0..3{frame(ppp,settings);}
            let start=std::time::Instant::now();let mut bytes=0usize;
            for _ in 0..60 {for output in frame(ppp,settings){
                bytes+=output.textures_delta.set.iter().map(|(_,delta)|delta.image.width()*delta.image.height()*4).sum::<usize>();
            }}
            println!("缩放={ppp:.2} 设置={settings} 60帧上传={bytes}字节 耗时={:?}",start.elapsed());
            assert_eq!(bytes,0,"大小稳定后不能重复上传字形图集，包括设置同时打开时");
            let textures=ctx.tex_manager();let textures=textures.read();
            let fonts=textures.allocated().filter(|(_,meta)|meta.name.starts_with("egui_font_texture")).count();
            assert!(fonts<=if settings&&ppp!=1.0{2}else{1},"旧倍率的字体贴图应释放：{fonts}");
        }
    }
}

#[test]
fn focus_durations_support_direct_input_and_keep_the_current_timer(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.open_settings_category(2);h.hover(H::local(200.0,60.0),1.0);
    let remaining=h.app.pomo.remaining;
    for (label,input,expected,rest) in [("25 分钟","45",45,false),("5 分钟","12",12,true),("45 分钟","999",180,false),("12 分钟","0",1,true),("180 分钟","abc",180,false)]{
        h.cmds.clear();h.click_text(label);h.frame(vec![]);
        assert!(h.app.is_editing(),"单击时长应进入输入状态");
        assert!(h.cmds.iter().any(|cmd|matches!(cmd,crate::app::Cmd::Focus)),"岛内输入必须取得原生键盘焦点");
        h.frame(vec![egui::Event::Text(input.into())]);
        h.frame(vec![egui::Event::Key{key:egui::Key::Enter,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
        h.frame(vec![]);
        let settings=crate::model::Settings::load();
        assert_eq!(if rest{settings.rest_minutes}else{settings.focus_minutes},expected,"输入需校验范围并保存");
        assert!(!h.app.is_editing());assert_eq!(h.app.pomo.remaining,remaining,"调整时长不能重置当前计时");
    }
    h.click_text("180 分钟");h.frame(vec![]);h.frame(vec![egui::Event::Text("30".into())]);
    h.frame(vec![egui::Event::Key{key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
    h.frame(vec![]);assert_eq!(h.app.settings.focus_minutes,180,"Esc 应取消本次输入");
    h.click_text("180 分钟");h.frame(vec![]);h.frame(vec![egui::Event::Text("50".into())]);
    h.click(H::local(268.0,62.0));h.frame(vec![]);
    assert_eq!(h.app.settings.focus_minutes,51,"点击加号应先保存输入，再增加一分钟");
    assert_eq!(crate::model::Settings::load().focus_minutes,51);
    h.click_text("51 分钟");h.frame(vec![]);h.frame(vec![egui::Event::Text("40".into())]);
    h.click(H::local(50.0,65.0));h.frame(vec![]);
    assert_eq!(h.app.settings.focus_minutes,40,"点击别处应保存输入");
    assert!(!h.app.is_editing());
}

#[test]
fn enlarged_island_preserves_font_resolution_with_a_separate_settings_window(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.open_settings_category(2);h.hover(H::local(200.0,60.0),1.0);
    h.app.open_settings();h.app.settings_tab=2;h.ctx.set_embed_viewports(false);
    let captured=std::sync::Arc::new(Mutex::new(None));let capture=captured.clone();
    egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
        let id=viewport.ids.this;
        let input=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(690.0,560.0))),
            viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()};
        *capture.lock().unwrap()=Some(ctx.run(input,|ctx|(viewport.viewport_ui_cb)(ctx)));
    });
    let mut raster=crate::preview_raster::Raster::new(WIN_W as usize,WIN_H as usize);
    raster.update_textures(&egui::TexturesDelta{set:vec![(egui::TextureId::default(),egui::epaint::ImageDelta::full(h.ctx.fonts(|fonts|fonts.image()),egui::TextureOptions::LINEAR))],..Default::default()});
    for scale in [1.0,1.2,1.4] {
        raster.w=(WIN_W*scale) as usize;raster.h=(WIN_H*scale) as usize;raster.buf.resize(raster.w*raster.h,[0.0;4]);
        for frame in 0..3 {
            h.t+=1.0/60.0;
            let input=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(crate::app::CANVAS_W,WIN_H))),
                time:Some(h.t),events:vec![egui::Event::PointerMoved(H::local(70.0,40.0))],viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(scale),..Default::default()})).collect(),..Default::default()};
            let root=h.ctx.run(input,|ctx|{
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{
                    let win=Rect::from_min_size(ui.max_rect().min,Vec2::new(WIN_W,WIN_H));
                    h.app.frame(ui,win,&Env{t:h.t,cursor:Some(H::local(70.0,40.0)),clock:"09:41:00".into(),..Default::default()});
                });
                h.app.settings_ui(ctx,false);
            });
            let child=captured.lock().unwrap().take().unwrap();
            assert_eq!(child.pixels_per_point,1.0,"设置窗口不跟随岛体变大");
            for (shapes,ppp) in [(&root.shapes,scale),(&child.shapes,1.0)] {
                for shape in shapes {if let egui::Shape::Text(text)=&shape.shape {
                    assert!((text.galley.pixels_per_point-ppp).abs()<0.0001,"各窗口应使用各自物理尺寸的字形");
                }}
            }
            raster.update_textures(&child.textures_delta);raster.update_textures(&root.textures_delta);
            if frame==2 {if let Some(dest)=std::env::var_os("LINGQI_FOCUS_SCALE_PREVIEW") {
                let dest=std::path::PathBuf::from(dest);std::fs::create_dir_all(&dest).unwrap();
                raster.buf.fill([0.18,0.20,0.24,1.0]);raster.draw(&h.ctx.tessellate(root.shapes,root.pixels_per_point),root.pixels_per_point);
                let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
                image::save_buffer(dest.join(format!("focus-size-{}.png",(scale*100.0) as u32)),&rgba,(WIN_W*scale) as u32,(WIN_H*scale) as u32,image::ColorType::Rgba8).unwrap();
            }}
            raster.free_textures(&root.textures_delta);
            raster.free_textures(&child.textures_delta);
        }
    }
}

#[test]
fn collapsed_resident_uses_its_outline_with_fewer_components() {
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.glass_blur=false;
    let size=Vec2::new(WIN_W,WIN_H);
    for frame in 0..90 {
        let t=frame as f64/60.0;
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,size)),time:Some(t),..Default::default()};
        let _=h.ctx.run(raw,|ctx|{
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{
                let out=h.app.frame(ui,Rect::from_min_size(ui.max_rect().min,size),&Env{t,..Default::default()});
                if frame==89 {
                    assert!(out.hit.width()<crate::app::PW);
                    assert!(out.resident_clip.is_some(),"收起后透明留白应放行鼠标");
                    assert!(out.glass.is_none());
                }
            });
        });
    }
}

#[test]
fn opaque_surface_gets_a_contour_with_every_skin_and_dock(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for skin in crate::model::Skin::ALL {for dock in crate::model::Dock::ALL {for island_shape in crate::model::IslandShape::ALL {for opaque_surface in [false,true] {
        let mut h=H::new();h.app.settings.skin=skin;h.app.settings.dock=dock;h.app.settings.island_shape=island_shape;h.app.settings.glass_blur=false;
        let size=if dock.is_side(){Vec2::new(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H)}else{Vec2::new(WIN_W,WIN_H)};
        let mut outlined=false;
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,size)),..Default::default()};
        let _=h.ctx.run(raw,|ctx|{
            egui::CentralPanel::default().show(ctx,|ui|{
                let win=Rect::from_min_size(ui.max_rect().min,size);
                let out=h.app.frame(ui,win,&Env{opaque_surface,..Default::default()});
                outlined=out.glass.is_some_and(|shapes|!shapes.is_empty());
            });
        });
        assert_eq!(outlined,opaque_surface,"皮肤={skin:?}，停靠={dock:?}，形状={island_shape:?}");
    }}}}
}

#[test]
fn regression_blur_follows_its_switch_even_when_compat_rendering_needs_the_contour(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for skin in crate::model::Skin::ALL {for glass_blur in [false,true] {for opaque_surface in [false,true] {
        let mut h=H::new();h.app.settings.skin=skin;h.app.settings.glass_blur=glass_blur;
        let size=Vec2::new(WIN_W,WIN_H);
        let mut blur=None;
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,size)),..Default::default()};
        let _=h.ctx.run(raw,|ctx|{
            egui::CentralPanel::default().show(ctx,|ui|{
                let out=h.app.frame(ui,Rect::from_min_size(ui.max_rect().min,size),&Env{opaque_surface,..Default::default()});
                blur=Some(out.blur);
            });
        });
        assert_eq!(blur,Some(skin.is_glass() && glass_blur),"皮肤={skin:?}，模糊={glass_blur}，兼容渲染={opaque_surface}");
    }}}
}


#[test]
fn top_bar_modes_preserve_legacy_settings_and_save_the_selected_display_mode(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let _h=H::new();
    use crate::model::Settings;
    let path=crate::model::config_dir().join("settings.txt");std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bounds=crate::display_layout::Bounds{left:-1920,top:-400,right:0,bottom:680};
    let display=crate::display_layout::Display{id:"test".into(),label:"测试屏".into(),bounds,work:crate::display_layout::Bounds{top:-368,..bounds},auto_bottom:0,primary:true,dpi:96};
    for enabled in [false,true] {
        std::fs::write(&path,format!("avoid_top_bar={}\n",enabled as u8)).unwrap();
        let mut settings=Settings::load();
        assert_eq!(settings.avoid_top_bar,enabled);assert!(!settings.top_bar_on_top,"旧配置默认沿用避让方式");
        for on_top in [false,true] {
            settings.top_bar_on_top=on_top;settings.save();let saved=Settings::load();
            assert_eq!((saved.avoid_top_bar,saved.top_bar_on_top),(enabled,on_top));
            assert_eq!(crate::display_layout::placement_bounds(&display,saved.avoids_top_bar()).top,if enabled&&!on_top{-368}else{-400},"置顶方式应回到屏幕顶边；关闭适配后不避让");
        }
    }
}

#[test]
fn top_bar_settings_switch_modes_and_respect_the_master_switch(){
    fn draw(h:&mut H,events:Vec<egui::Event>,raster:&mut crate::preview_raster::Raster)->egui::FullOutput {
        h.t+=0.1;let time=h.t;
        let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            let raw=egui::RawInput{viewport_id:id,time:Some(time),focused:true,events:events.clone(),screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,1800.0))),viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()};
            *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        let root=h.ctx.run(Default::default(),|ctx|h.app.settings_ui(ctx,false));raster.update_textures(&root.textures_delta);
        let child=output.lock().unwrap().take().unwrap();raster.update_textures(&child.textures_delta);child
    }
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for dark in [true,false] {
        let mut h=H::new();h.app.open_settings();h.app.settings_tab=crate::extras::SETTINGS_TAB_POSITION;h.ctx.set_embed_viewports(false);
        h.app.settings.settings_theme=if dark{crate::settings_theme::SettingsTheme::Dark}else{crate::settings_theme::SettingsTheme::Light};
        let mut raster=crate::preview_raster::Raster::new(690,1800);
        for _ in 0..3{let _=draw(&mut h,vec![],&mut raster);}
        for (on_top,label) in [(true,"灵栖置顶"),(false,"避开顶栏")] {
            let output=draw(&mut h,vec![],&mut raster);
            let position=output.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()==label=>{
                assert!(s.clip_rect.contains_rect(t.visual_bounding_rect()),"显示方式按钮不能裁切");Some(t.visual_bounding_rect().center())
            },_=>None}).expect("显示方式应提供避让和置顶两个选项");
            for pressed in [true,false] {let _=draw(&mut h,vec![egui::Event::PointerMoved(position),egui::Event::PointerButton{pos:position,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}],&mut raster);}
            assert_eq!(h.app.settings.top_bar_on_top,on_top);assert_eq!(crate::model::Settings::load().top_bar_on_top,on_top,"切换后立即保存");
            if let Some(dest)=std::env::var_os("LINGQI_TOP_BAR_PREVIEW_DIR") {
                let output=draw(&mut h,vec![],&mut raster);raster.buf.fill(if dark{[0.08,0.08,0.09,1.0]}else{[0.96,0.96,0.97,1.0]});
                raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
                let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);let dest=std::path::PathBuf::from(dest);std::fs::create_dir_all(&dest).unwrap();
                image::save_buffer(dest.join(format!("top-bar-{}-{}.png",if dark{"dark"}else{"light"},if on_top{"on-top"}else{"avoid"})),&rgba,690,1800,image::ColorType::Rgba8).unwrap();
            }
        }
        h.app.settings.avoid_top_bar=false;
        let output=draw(&mut h,vec![],&mut raster);
        assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()=="灵栖置顶")),"关闭适配后收起显示方式选项");
    }
}
















#[test]
fn settings_keep_dark_buttons_when_system_theme_changes() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let h=H::new();
    for theme in [egui::Theme::Light,egui::Theme::Dark,egui::Theme::Light] {
        let _=h.ctx.run(egui::RawInput{system_theme:Some(theme),..Default::default()},|ctx| {
            assert!(ctx.style().visuals.widgets.inactive.weak_bg_fill.r()<100,"系统浅色模式不能把深色设置里的按钮变成白底");
        });
    }
}





#[test]
fn glass_pill_rim_stays_on_outline_while_collapsing() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for ppp in [1.0,1.25,1.5,2.0] {
        let mut h=H::new();
        h.app.settings.skin=crate::model::Skin::GlassLight;
        h.app.settings.island_shape=crate::model::IslandShape::Pill;
        h.ctx.set_zoom_factor(ppp);
        h.hover(Pos2::new(WIN_W/2.0,15.0),1.5);
        assert!(h.app.progress()>0.99);
        h.cursor=None;
        let mut raster=crate::preview_raster::Raster::new((crate::app::CANVAS_W*ppp) as usize,(WIN_H*ppp) as usize);
        let mut checked=0;
        for frame in 0..100 {
            h.frame(vec![egui::Event::PointerGone]);
            let rims=h.shapes.iter().filter_map(|s|match &s.shape {
                egui::Shape::Rect(r) if r.rect.width()>100.0 && r.rect.height()>=10.0=>Some((s.clone(),r.rect,if r.stroke.is_empty(){r.fill.a() as f32/255.0}else{0.0})),
                _=>None,
            }).collect::<Vec<_>>();
            for (shape,rect,alpha) in rims {
                checked+=1;
                raster.buf.fill([0.0;4]);
                raster.draw(&h.ctx.tessellate(vec![shape],ppp),ppp);
                let y=(rect.center().y*ppp).floor() as usize;
                let left=((rect.left()+4.0)*ppp).ceil() as usize;
                let right=((rect.right()-4.0)*ppp).floor() as usize;
                for x in left..right {
                    assert!((raster.buf[y*raster.w+x][3]-alpha).abs()<0.02,"胶囊中心出现接缝：ppp={ppp}, frame={frame}, rect={rect:?}, x={x}, alpha={alpha}, actual={}",raster.buf[y*raster.w+x][3]);
                }
            }
        }
        assert!(checked>=100,"应覆盖完整收起动画");
    }
}

#[test]
fn oled_shift_waits_for_the_panel_to_finish_closing() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.oled_shift=true;
    h.app.show_page(1);
    h.hover(H::local(200.0,60.0),5.0);
    assert!(h.app.open);
    h.cursor=Some(Pos2::new(600.0,180.0));
    for _ in 0..120 {
        h.frame(vec![]);
        if !h.app.open {break;}
    }
    assert!(!h.app.open);
    assert!(h.app.progress()>0.1,"测试应停在收起动画尚未完成时");
    assert!(!h.app.oled_shift_idle(&h.ctx),"收起动画仍有 {:.1}% 展开程度时不能恢复位移",h.app.progress()*100.0);
    h.hover(Pos2::new(600.0,180.0),1.0);
    assert!(h.app.oled_shift_idle(&h.ctx),"动画完成后应恢复位移");
}

#[test]
fn oled_shift_pauses_during_hover_intent() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.oled_shift=true;
    h.hover(Pos2::new(600.0,180.0),1.0);
    assert!(h.app.oled_shift_idle(&h.ctx));
    h.hover(Pos2::new(WIN_W/2.0,12.0),0.05);
    assert!(!h.app.open,"悬停意图缓冲期间尚未展开");
    assert!(h.hit.contains(Pos2::new(WIN_W/2.0,12.0)),"鼠标已进入岛体交互区域");
    assert!(!h.app.oled_shift_idle(&h.ctx),"悬停交互期间不能移动岛体");
}

#[test]
fn oled_shift_pauses_on_the_first_pointer_press_frame() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();let away=Pos2::new(600.0,180.0);
    h.hover(away,1.0);assert!(h.app.oled_shift_idle(&h.ctx));
    h.frame(vec![egui::Event::PointerButton{pos:away,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
    assert!(!h.app.oled_shift_idle(&h.ctx),"按下的首帧必须暂停位移");
    h.frame(vec![]);assert!(!h.app.oled_shift_idle(&h.ctx),"按住期间必须暂停位移");
    h.frame(vec![egui::Event::PointerButton{pos:away,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
    assert!(h.app.oled_shift_idle(&h.ctx),"松开且没有交互后应恢复位移");
}


struct H {
    ctx: egui::Context,
    app: App,
    t: f64,
    cursor: Option<Pos2>,
    cmds:Vec<crate::app::Cmd>,
    region:Rect,
    hit:Rect,
    top_bar_notice:Option<Rect>,
    animating:bool,
    interactive_transition:bool,
    shapes:Vec<egui::epaint::ClippedShape>,
    power: (Option<u8>, bool),
    network: Option<bool>,
    system_idle: Option<f64>,
    screen_locked: bool,
    fullscreen: bool,
    browser_at_top: Option<bool>,
    file_drop_zone:Option<Rect>,
    hovered_files:Vec<egui::HoveredFile>,
    dropped_files:Vec<egui::DroppedFile>,
}

impl H {
    fn new() -> Self {Self::with_data(None)}
    /// `seed` 里的文件先复制进测试数据目录，用来按某份真实设置复现画面。
    fn with_data(seed:Option<&std::path::Path>) -> Self {
        let dir = std::env::temp_dir().join(format!("lingqi-test-{}-{}", std::process::id(), rand_id()));
        if let Some(seed)=seed {
            std::fs::create_dir_all(&dir).unwrap();
            for entry in std::fs::read_dir(seed).unwrap().flatten() {
                if entry.path().is_file() {std::fs::copy(entry.path(),dir.join(entry.file_name())).unwrap();}
            }
        }
        std::env::set_var("LINGQI_DATA_DIR", &dir);
        let ctx = egui::Context::default();
        let app = App::new(&ctx, Options { pet_idle: 1e9 });
        Self { ctx, app, t: 0.0, cursor: None,cmds:vec![],region:Rect::NOTHING,hit:Rect::NOTHING,top_bar_notice:None,animating:false,interactive_transition:false,shapes:Vec::new(),power:(None,false),network:None,system_idle:None,screen_locked:false,fullscreen:false,browser_at_top:None,file_drop_zone:None,hovered_files:vec![],dropped_files:vec![] }
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.t += 1.0 / 60.0;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(crate::app::CANVAS_W, WIN_H))),
            time: Some(self.t),
            events,
            hovered_files:self.hovered_files.clone(),
            dropped_files:std::mem::take(&mut self.dropped_files),
            focused: true,
            ..Default::default()
        };
        let env = Env {
            t: self.t,
            clock: "09:41:00".into(),
            today: "2026-10-28".into(),
            date: "10月28日 周三".into(),
            cursor: self.cursor,

            battery:self.power.0,
            charging:self.power.1,
            network:self.network,
            idle_seconds:self.system_idle,
            screen_locked:self.screen_locked,
            fullscreen_hidden:self.fullscreen && self.app.settings.hide_fullscreen,
            browser_at_top:self.browser_at_top,
            ..Default::default()
        };
        let cmds=&mut self.cmds;
        let region=&mut self.region;
        let hit=&mut self.hit;
        let top_bar_notice=&mut self.top_bar_notice;
        let animating=&mut self.animating;
        let interactive_transition=&mut self.interactive_transition;
        let app = &mut self.app;
        let file_drop_zone=&mut self.file_drop_zone;
        let output = self.ctx.run(raw, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx, |ui| {
                let win = Rect::from_min_size(ui.max_rect().min,Vec2::new(WIN_W,WIN_H));
                let frame=app.frame(ui, win, &env);*region=frame.region;*hit=frame.hit;*top_bar_notice=frame.top_bar_notice;*animating=frame.animating;*interactive_transition=frame.interactive_transition;*file_drop_zone=frame.file_drop_zone;cmds.extend(frame.cmds);
            });
        });
        self.shapes=output.shapes;
    }
    fn hover(&mut self, p: Pos2, secs: f64) {
        self.cursor = Some(p);
        let n = (secs * 60.0) as usize;
        for _ in 0..n {
            self.frame(vec![egui::Event::PointerMoved(p)]);
        }
    }
    fn click(&mut self, p: Pos2) {
        self.cursor = Some(p);
        self.frame(vec![egui::Event::PointerMoved(p)]);
        self.frame(vec![egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        self.frame(vec![egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
    }
    fn click_text(&mut self,label:&str){
        if !self.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()==label))&&self.app.notice.is_some(){self.click_notice_badge();}
        let point=self.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()==label&&t.opacity_factor>0.9&&s.clip_rect.contains(t.visual_bounding_rect().center())=>Some(t.visual_bounding_rect().center()),_=>None}).unwrap_or_else(||panic!("未找到可点击文字：{label}"));
        self.click(point);
    }
    fn notice_button(&self)->Rect {self.ctx.read_response(egui::Id::new(("card",true,"notice-badge"))).expect("应显示通知入口").rect}
    fn click_notice_badge(&mut self){if self.app.compact_notices(){self.click(self.notice_button().center());}}
    fn wheel(&mut self,delta:f32) {
        self.frame(vec![egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:Vec2::new(0.0,delta),modifiers:Default::default()}]);
    }
    fn picker_frame(&mut self,events:Vec<egui::Event>){
        self.frame(events);
    }
    fn picker_click(&mut self,label:&str){
        for _ in 0..60 {self.picker_frame(vec![]);if (self.app.progress()-1.0).abs()<0.01 {break;}}
        self.picker_frame(vec![]);
        let find=|h:&H|h.shapes.iter().find_map(|shape|match &shape.shape{
            egui::Shape::Text(text) if text.galley.text()==label && shape.clip_rect.contains(text.visual_bounding_rect().center())=>Some(text.visual_bounding_rect().center()),_=>None,
        });
        let point=find(self).unwrap_or_else(||panic!("未找到可见选项：{label}"));
        assert!(self.hit.contains(point),"选项 {label} 超出岛体命中区域");
        self.picker_frame(vec![egui::Event::PointerMoved(point)]);
        for pressed in [true,false]{self.picker_frame(vec![egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}]);}
    }
    fn local(x: f32, y: f32) -> Pos2 {
        Pos2::new((WIN_W - PW) / 2.0 + x, y)
    }
    fn quota_centers(&self)->Vec<f32> {
        fn collect(shape:&egui::Shape,out:&mut Vec<f32>) {
            match shape {
                egui::Shape::Vec(shapes)=>for shape in shapes {collect(shape,out);},
                egui::Shape::Path(path) if path.points.len()>50=>{
                    let min=path.points.iter().map(|p|p.x).fold(f32::INFINITY,f32::min);
                    let max=path.points.iter().map(|p|p.x).fold(f32::NEG_INFINITY,f32::max);
                    if (20.0..28.0).contains(&(max-min)) {out.push((min+max)/2.0);}
                },
                egui::Shape::Circle(circle) if (10.0..14.0).contains(&circle.radius) && circle.stroke.width>0.0 && (circle.center.y-15.0).abs()<0.5=>out.push(circle.center.x),
                _=>{},
            }
        }
        let mut out=Vec::new();for shape in &self.shapes {collect(&shape.shape,&mut out);}
        out.sort_by(f32::total_cmp);out
    }
}

fn rand_id() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64
}

fn island_drag_frame(h:&mut H,bounds:crate::display_layout::Bounds,ppp:f32,pointer:Option<Pos2>,pressed:Option<bool>)->Rect {
    use crate::{display_layout,side_dock};
    let dock=h.app.settings.dock;
    let (width,height)=if dock.is_side(){(side_dock::SIDE_W,side_dock::SIDE_H)}else{(WIN_W,WIN_H)};
    let offset=h.app.placement_offset();
    let (x,y)=if dock.is_side(){display_layout::side_origin(bounds,dock,(width,height),side_dock::SIDE_HALF_EXTENT,ppp,offset)}
        else{display_layout::horizontal_origin(bounds,dock,ppp,offset)}.unwrap();
    let screen=Rect::from_min_max(Pos2::new((bounds.left-x) as f32/ppp,(bounds.top-y) as f32/ppp),Pos2::new((bounds.right-x) as f32/ppp,(bounds.bottom-y) as f32/ppp));
    let cursor=pointer.map(|p|p+screen.min.to_vec2());
    let mut events=cursor.map(|p|vec![egui::Event::PointerMoved(p)]).unwrap_or_default();
    if let (Some(pos),Some(pressed))=(cursor,pressed){events.push(egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()});}
    h.t+=1.0/60.0;
    let env=Env{t:h.t,clock:"09:41:00".into(),today:"2026-10-28".into(),date:"10月28日 周三".into(),cursor,desktop_cursor:pointer.map(|p|(bounds.left+(p.x*ppp).round() as i32,bounds.top+(p.y*ppp).round() as i32)),screen:Some(screen),..Default::default()};
    let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(crate::app::CANVAS_W.max(width),height))),time:Some(h.t),events,focused:true,
        viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(ppp),..Default::default()})).collect(),..Default::default()};
    let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{
        let out=h.app.frame(ui,Rect::from_min_size(ui.max_rect().min,Vec2::new(width,height)),&env);h.region=out.region;h.hit=out.hit;h.cmds.extend(out.cmds);
    });});
    h.shapes=output.shapes;
    screen
}

fn island_drag_header(h:&mut H,bounds:crate::display_layout::Bounds,ppp:f32)->Pos2 {
    h.app.show_page(1);
    let mut screen=Rect::NOTHING;
    for _ in 0..90{screen=island_drag_frame(h,bounds,ppp,None,None);}
    h.shapes.iter().find_map(|shape|match &shape.shape{
        egui::Shape::Text(text) if text.galley.text()=="10月28日 周三"=>Some(text.visual_bounding_rect().center()-screen.min.to_vec2()),_=>None,
    }).expect("展开后的日期应可见")
}

#[test]
fn bottom_dock_hover_opens_upward_with_stable_edge_and_visible_header() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let win=Rect::from_min_size(Pos2::ZERO,Vec2::new(WIN_W,WIN_H));
    for shape in crate::model::IslandShape::ALL {
        let mut h=H::new();
        h.app.settings.dock=crate::model::Dock::Bottom;
        h.app.settings.island_shape=shape;
        for _ in 0..60 {h.frame(vec![]);}
        let body=|h:&H|h.app.island_rect(win,h.app.w.pos,h.app.h.pos);
        let start=body(&h);
        let pointer=win.center_bottom()-Vec2::Y;
        assert!(h.hit.contains(pointer) && !h.hit.contains(win.center_top()));
        h.cursor=Some(pointer);
        let mut animated=false;
        for _ in 0..100 {
            h.frame(vec![]);
            let panel=body(&h);
            assert!((panel.bottom()-start.bottom()).abs()<0.01);
            animated|=panel.top()<start.top()-4.0 && panel.height()<h.app.expanded_height()-4.0;
        }
        let panel=body(&h);
        assert!(animated && h.app.open && panel.top()<start.top()-60.0);
        assert!(h.region.contains(panel.center()) && h.hit.contains(panel.center()));
        assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)
            if t.galley.text()=="10月28日 周三" && s.clip_rect.contains(t.visual_bounding_rect().center()))));
        h.cursor=None;
        for _ in 0..120 {h.frame(vec![]);}
        assert!(!h.app.open && (body(&h).top()-start.top()).abs()<1.0);
    }
}

#[test]
fn island_position_drag_crosses_shared_edge_and_persists_target(){
    use crate::{display_layout::{Bounds,Display},model::{Dock,Settings}};
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let a=Bounds{left:-1920,top:0,right:0,bottom:1080};
    let b=Bounds{left:0,top:-240,right:2560,bottom:1200};
    let mut h=H::new();
    h.app.displays=vec![
        Display{id:"A".into(),label:"左屏".into(),bounds:a,work:a,auto_bottom:0,primary:true,dpi:96},
        Display{id:"B".into(),label:"右屏".into(),bounds:b,work:b,auto_bottom:0,primary:false,dpi:144},
    ];
    h.app.settings.display_id="A".into();
    h.app.settings.save();
    let start=island_drag_header(&mut h,a,1.0);
    island_drag_frame(&mut h,a,1.0,Some(start),Some(true));
    island_drag_frame(&mut h,a,1.0,Some(Pos2::new(1918.0,700.0)),None);
    island_drag_frame(&mut h,a,1.0,Some(Pos2::new(1924.0,700.0)),None);
    assert_eq!((h.app.settings.display_id.as_str(),h.app.settings.dock),("A",Dock::Right));
    island_drag_frame(&mut h,a,1.0,Some(Pos2::new(1960.0,700.0)),None);
    assert_eq!((h.app.settings.display_id.as_str(),h.app.settings.dock),("B",Dock::Left));
    assert_eq!(Settings::load().display_id,"A");
    let offset=h.app.settings.island_offset;
    let travel=(1440.0/1.5/2.0)-crate::side_dock::SIDE_HALF_EXTENT;
    assert_eq!(offset,((940.0_f32/1.5-480.0)/travel*100.0).round() as i32,"使用右屏的 DPI 和虚拟桌面原点");
    let on_b=Pos2::new(40.0/1.5,940.0/1.5);
    island_drag_frame(&mut h,b,1.5,Some(on_b),None);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::LineSegment{points,stroke}
        if stroke.width==2.5 && points[0].x==points[1].x && points[0].x<24.0 && stroke.color.a()>0)),
        "交接后在右屏左边缘画出短亮线");
    island_drag_frame(&mut h,b,1.5,Some(on_b),Some(false));
    let saved=Settings::load();
    assert_eq!((saved.display_id.as_str(),saved.dock,saved.island_offset),("B",Dock::Left,offset));
    for disconnect in [false,true] {
        let start=island_drag_header(&mut h,b,1.5);
        island_drag_frame(&mut h,b,1.5,Some(start),Some(true));
        island_drag_frame(&mut h,b,1.5,Some(Pos2::new(-4.0,600.0)),None);
        assert_eq!(h.app.settings.display_id,"B","反向轻微越界仍可贴右屏左侧");
        island_drag_frame(&mut h,b,1.5,Some(Pos2::new(-80.0,600.0)),None);
        assert_eq!((h.app.settings.display_id.as_str(),h.app.settings.dock),("A",Dock::Right));
        if disconnect {
            h.app.displays.retain(|d|d.id!="A");
            island_drag_frame(&mut h,b,1.5,Some(on_b),Some(false));
        }else{
            h.frame(vec![egui::Event::Key{key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
            island_drag_frame(&mut h,b,1.5,Some(on_b),Some(false));
        }
        assert_eq!((h.app.settings.display_id.as_str(),h.app.settings.dock,h.app.settings.island_offset),("B",Dock::Left,offset),"取消或拔屏应恢复整组原始位置");
    }
}

#[test]
fn island_position_drag_tracks_screen_coordinates_and_saves_on_release(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let bounds=crate::display_layout::Bounds{left:-2560,top:-400,right:0,bottom:1040};
    for dock in crate::model::Dock::ALL {for ppp in [1.0,1.5] {
        let mut h=H::new();h.app.settings.dock=dock;h.app.settings.island_offset=-40;h.app.settings.save();
        // 起点离居中足够远，90 点位移不会落进居中吸附范围。
        let start=island_drag_header(&mut h,bounds,ppp);
        let before=island_drag_frame(&mut h,bounds,ppp,Some(start),Some(true));
        let delta=if dock.is_side(){Vec2::new(0.0,90.0)}else{Vec2::new(90.0,0.0)};
        let end=start+delta;
        island_drag_frame(&mut h,bounds,ppp,Some(end),None);
        assert!(h.app.settings.island_offset>-40,"{dock:?} / {ppp}：拖动应同步现有位置参数");
        assert_eq!(crate::model::Settings::load().island_offset,-40,"拖动途中不能反复写配置");
        let offset=h.app.settings.island_offset;
        for _ in 0..30{island_drag_frame(&mut h,bounds,ppp,Some(end),None);}
        let after=island_drag_frame(&mut h,bounds,ppp,Some(end),None);
        let movement=before.min-after.min;
        assert!(((if dock.is_side(){movement.y}else{movement.x})-90.0).abs()<6.0,"{dock:?} / {ppp}：窗口应跟随实际鼠标位移，允许整数位置刻度带来的取整误差");
        assert_eq!(if dock.is_side(){movement.x}else{movement.y},0.0,"拖动不能脱离停靠边缘");
        assert_eq!(h.app.settings.island_offset,offset,"窗口跟随鼠标移动后，原地持有不能发生累积漂移");
        island_drag_frame(&mut h,bounds,ppp,Some(end),Some(false));
        assert_eq!(crate::model::Settings::load().island_offset,offset,"松手后应保存位置");
        assert_eq!((h.app.settings.dock,h.app.settings.display_id.as_str()),(dock,""));
    }}
}

#[test]
fn island_position_drag_clamps_to_existing_limits_and_stays_resident(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let bounds=crate::display_layout::Bounds{left:0,top:0,right:1920,bottom:1080};
    for dock in crate::model::Dock::ALL {for shape in crate::model::IslandShape::ALL {
        let mut h=H::new();h.app.settings.dock=dock;h.app.settings.island_shape=shape;
        let start=island_drag_header(&mut h,bounds,1.0);
        island_drag_frame(&mut h,bounds,1.0,Some(start),Some(true));
        let side_x=if dock==crate::model::Dock::Left {0.0}else{1919.0};
        let end=if dock.is_side(){Pos2::new(side_x,1079.0)}else{Pos2::new(1855.0,start.y)};
        for _ in 0..120{island_drag_frame(&mut h,bounds,1.0,Some(end),None);}
        assert!(!h.app.open,"{dock:?} 拖动期间应保持常驻形态");
        assert!(h.app.progress()<0.01,"拖动期间不应保留展开面板");
        assert!(!h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="10月28日 周三")),"拖动期间不应显示展开顶栏");
        assert_eq!(h.app.settings.island_offset,crate::model::ISLAND_OFFSET_MAX);
        let end=if dock.is_side(){Pos2::new(side_x,65.0)}else{Pos2::new(65.0,start.y)};
        island_drag_frame(&mut h,bounds,1.0,Some(end),None);
        assert_eq!(h.app.settings.island_offset,-crate::model::ISLAND_OFFSET_MAX);
        assert_eq!(h.app.settings.dock,dock);
        island_drag_frame(&mut h,bounds,1.0,Some(end),Some(false));
        let offset=h.app.settings.island_offset;
        for _ in 0..40{island_drag_frame(&mut h,bounds,1.0,Some(start),None);}
        assert_eq!(h.app.settings.island_offset,offset,"松手后不能继续跟着鼠标移动");
    }}
}

#[test]
fn bottom_dock_drag_snaps_at_taskbar_top_before_cursor_enters_taskbar() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    use crate::{display_layout::{Bounds,Display},model::Dock};
    let bounds=Bounds{left:0,top:0,right:1920,bottom:1080};
    let work=Bounds{bottom:968,..bounds};
    let mut h=H::new();
    h.app.displays=vec![Display{id:"primary".into(),label:String::new(),primary:true,dpi:96,bounds,work,auto_bottom:0}];
    let start=island_drag_header(&mut h,bounds,1.0);
    island_drag_frame(&mut h,bounds,1.0,Some(start),Some(true));
    let end=Pos2::new(1050.0,work.bottom as f32-8.0);
    island_drag_frame(&mut h,bounds,1.0,Some(end),None);
    assert_eq!(h.app.settings.dock,Dock::Bottom);
    island_drag_frame(&mut h,work,1.0,Some(end),Some(false));
    assert_eq!(crate::model::Settings::load().dock,Dock::Bottom);
}

#[test]
fn island_position_drag_below_a_top_bar_switches_edges_like_the_bare_screen(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    use crate::model::{Dock,Settings};
    let monitor=crate::display_layout::Bounds{left:0,top:0,right:1920,bottom:1080};
    let display=crate::display_layout::Display{id:"d".into(),label:"d".into(),bounds:monitor,work:crate::display_layout::Bounds{top:28,..monitor},auto_bottom:0,primary:true,dpi:96};
    let bounds=crate::display_layout::placement_bounds(&display,true);
    for to in [Dock::Left,Dock::Right] {
        let mut h=H::new();h.app.settings.save();
        let start=island_drag_header(&mut h,bounds,1.0);
        island_drag_frame(&mut h,bounds,1.0,Some(start),Some(true));
        let size=Vec2::new(1920.0,1052.0);
        let end=if to==Dock::Left{Pos2::new(0.0,size.y*0.6)}else{Pos2::new(size.x,size.y*0.6)};
        for _ in 0..30{island_drag_frame(&mut h,bounds,1.0,Some(end),None);}
        assert_eq!(h.app.settings.dock,to);
        let back=Pos2::new(size.x*0.5,0.0);
        for _ in 0..30{island_drag_frame(&mut h,bounds,1.0,Some(back),None);}
        assert_eq!(h.app.settings.dock,Dock::Top,"拖回顶栏下沿应能回到顶部");
        island_drag_frame(&mut h,bounds,1.0,Some(back),Some(false));
        assert_eq!(Settings::load().dock,Dock::Top);
        assert!(Settings::load().avoid_top_bar,"拖动不改变避开顶栏设置");
    }
}

#[test]
fn island_position_drag_switches_between_all_four_edges_and_saves_on_release(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    use crate::model::{Dock,IslandShape,Settings};
    let bounds=crate::display_layout::Bounds{left:-2560,top:-400,right:0,bottom:1040};
    for from in Dock::ALL {for to in Dock::ALL.into_iter().filter(|to|*to!=from) {for ppp in [1.0,1.5] {for shape in IslandShape::ALL {
        let mut h=H::new();h.app.settings.dock=from;h.app.settings.island_shape=shape;h.app.settings.save();
        let start=island_drag_header(&mut h,bounds,ppp);
        island_drag_frame(&mut h,bounds,ppp,Some(start),Some(true));
        let size=Vec2::new(2560.0,1440.0)/ppp;
        let end=match to {Dock::Bottom=>Pos2::new(size.x*0.6,size.y),Dock::Top=>Pos2::new(size.x*0.6,0.0),Dock::Left=>Pos2::new(0.0,size.y*0.6),Dock::Right=>Pos2::new(size.x,size.y*0.6)};
        for _ in 0..30{island_drag_frame(&mut h,bounds,ppp,Some(end),None);}
        assert_eq!(h.app.settings.dock,to,"{from:?} → {to:?} / {ppp} / {shape:?}");
        assert!(!h.app.open && h.app.progress()<0.01,"切换停靠边后仍应显示常驻形态");
        assert_eq!(Settings::load().dock,from,"拖动中不能写入停靠设置");
        let offset=h.app.settings.island_offset;
        for _ in 0..15{island_drag_frame(&mut h,bounds,ppp,Some(end),None);}
        assert_eq!(h.app.settings.island_offset,offset,"换边后持有不能漂移");
        island_drag_frame(&mut h,bounds,ppp,Some(end),Some(false));
        let saved=Settings::load();assert_eq!((saved.dock,saved.island_offset),(to,offset));
        for _ in 0..30{island_drag_frame(&mut h,bounds,ppp,Some(end),None);}
        assert!(!h.app.open,"松手后应留在常驻形态");
        let screen=island_drag_frame(&mut h,bounds,ppp,Some(end),None);
        let resident=h.hit.center()-screen.min.to_vec2();
        for _ in 0..30{island_drag_frame(&mut h,bounds,ppp,Some(resident),None);}
        assert!(!h.app.open,"松手后光标留在岛上不能立即展开");
        for _ in 0..30{island_drag_frame(&mut h,bounds,ppp,None,None);}
        for _ in 0..60{island_drag_frame(&mut h,bounds,ppp,Some(resident),None);}
        assert!(h.app.open,"光标离开再悬停，应能正常展开");
        assert_eq!(h.app.settings.display_id,"");
    }}}}
}

#[test]
fn island_position_drag_keeps_the_edge_in_the_middle_and_does_not_flap_at_corners(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    use crate::model::Dock;
    let bounds=crate::display_layout::Bounds{left:0,top:0,right:1920,bottom:1080};
    let mut h=H::new();let start=island_drag_header(&mut h,bounds,1.0);
    island_drag_frame(&mut h,bounds,1.0,Some(start),Some(true));
    for (point,dock) in [(Pos2::new(500.0,500.0),Dock::Top),(Pos2::new(0.0,500.0),Dock::Left),
        (Pos2::new(960.0,540.0),Dock::Left),(Pos2::new(1250.0,540.0),Dock::Right),(Pos2::new(1000.0,540.0),Dock::Right),
        (Pos2::new(700.0,540.0),Dock::Left),(Pos2::new(4.0,3.0),Dock::Left),
        (Pos2::new(3.0,4.0),Dock::Left),(Pos2::new(100.0,0.0),Dock::Top),
        (Pos2::new(1920.0,500.0),Dock::Right),(Pos2::new(1918.0,3.0),Dock::Right),
        (Pos2::new(1500.0,0.0),Dock::Top)] {
        for _ in 0..5{island_drag_frame(&mut h,bounds,1.0,Some(point),None);}
        assert_eq!(h.app.settings.dock,dock,"光标={point:?}");
        assert!(!h.app.open);
    }
    island_drag_frame(&mut h,bounds,1.0,None,Some(false));
    island_drag_frame(&mut h,bounds,1.0,Some(start),Some(false));
    let dock=h.app.settings.dock;let offset=h.app.settings.island_offset;
    island_drag_frame(&mut h,bounds,1.0,Some(Pos2::new(0.0,500.0)),None);
    assert_eq!((h.app.settings.dock,h.app.settings.island_offset),(dock,offset),"松手后不能继续拖动");
}

#[test]
fn island_position_drag_lifts_on_hold_glides_smoothly_and_snaps_to_center(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let bounds=crate::display_layout::Bounds{left:0,top:0,right:1920,bottom:1080};
    let mut h=H::new();h.app.settings.island_offset=-30;h.app.settings.save();
    let start=island_drag_header(&mut h,bounds,1.0);
    island_drag_frame(&mut h,bounds,1.0,Some(start),Some(true));
    // 原地按住片刻即拿起：面板开始收起，位置不变。
    for _ in 0..20{island_drag_frame(&mut h,bounds,1.0,Some(start),None);}
    assert!(!h.app.open && h.app.progress()<0.5,"按住不动也应拿起岛体");
    assert_eq!(h.app.settings.island_offset,-30);
    // 窗口平滑跟随：一帧跳过去的光标，窗口要分几帧追上，且不按整数刻度跳。
    let travel=1920.0/2.0-crate::display_layout::ISLAND_HALF_EXTENT;
    let to=|units:f32|Pos2::new(start.x+(units+30.0)/100.0*travel,start.y);
    island_drag_frame(&mut h,bounds,1.0,Some(to(-10.0)),None);
    island_drag_frame(&mut h,bounds,1.0,Some(to(-10.0)),None);
    let mid=h.app.placement_offset();
    assert!(mid> -0.30 && mid< -0.10,"窗口应在途中，而不是瞬间到位：{mid}");
    for _ in 0..40{island_drag_frame(&mut h,bounds,1.0,Some(to(-10.0)),None);}
    assert!((h.app.placement_offset()+0.10).abs()<0.003,"持有后应追上光标");
    // 居中附近吸附，离开吸附范围后恢复跟随。
    assert!(h.app.center_guide>0.2 && h.app.center_guide<0.9 && h.app.center_lock_a<0.05,"接近居中时准线渐显，但还没点亮：{}",h.app.center_guide);
    island_drag_frame(&mut h,bounds,1.0,Some(to(2.0)),None);
    assert_eq!(h.app.settings.island_offset,0,"靠近居中应吸附");
    assert!(h.app.center_locked,"吸附后应标记为已居中");
    for _ in 0..30{island_drag_frame(&mut h,bounds,1.0,Some(to(2.0)),None);}
    assert!(h.app.center_guide>0.9 && h.app.center_lock_a>0.9,"居中时准线应清晰可见并切换为已居中配色");
    // 吸住后有滞回：稍微拖出一点仍停在中间，拖得更远才松开。
    island_drag_frame(&mut h,bounds,1.0,Some(to(7.0)),None);
    assert_eq!(h.app.settings.island_offset,0,"居中后轻微偏移不应松开");
    island_drag_frame(&mut h,bounds,1.0,Some(to(12.0)),None);
    assert_eq!(h.app.settings.island_offset,12);
    assert!(!h.app.center_locked);
    island_drag_frame(&mut h,bounds,1.0,Some(to(-28.5)),None);
    assert_eq!(h.app.settings.island_offset,-30,"靠近原位置应吸附回去");
    island_drag_frame(&mut h,bounds,1.0,Some(to(2.0)),None);
    island_drag_frame(&mut h,bounds,1.0,Some(to(2.0)),Some(false));
    assert_eq!(crate::model::Settings::load().island_offset,0,"松手保存吸附后的位置");
    for _ in 0..90{island_drag_frame(&mut h,bounds,1.0,Some(to(2.0)),None);}
    assert_eq!(h.app.placement_offset(),0.0,"落位后回到保存的位置");
}

#[test]
fn shelf_drop_zone_leaves_normal_hover_and_text_selection_unchanged(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    for _ in 0..90{h.frame(vec![]);}let zone=h.file_drop_zone.unwrap();
    assert_eq!(zone.width(),PW);assert_eq!(zone.height(),96.0);
    let point=zone.right_bottom()-Vec2::splat(12.0);assert!(!h.hit.contains(point));
    h.hover(point,0.8);assert!(!h.app.open);
    h.frame(vec![egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
    h.hover(point,0.8);assert!(!h.app.open,"按住鼠标拖选文字不应打开置物架");
    h.frame(vec![egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
    h.app.settings.visible_pages[3]=false;h.frame(vec![]);assert!(h.file_drop_zone.is_none());
}

#[test]
fn shelf_file_drag_opens_below_the_island_and_cancellation_restores_the_page(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    let saved=crate::model::config_dir().join("原有文件.txt");
    std::fs::create_dir_all(saved.parent().unwrap()).unwrap();
    std::fs::write(&saved,"test").unwrap();
    h.app.shelf.items.push(saved.clone());
    h.app.go(1,1.0);
    for _ in 0..90 {h.frame(vec![]);}
    let point=h.file_drop_zone.unwrap().right_bottom()-Vec2::splat(12.0);
    assert!(!h.hit.contains(point));
    h.cursor=Some(point);
    h.hovered_files.push(egui::HoveredFile{path:Some(crate::model::config_dir().join("拖入文件.txt")),..Default::default()});
    h.frame(vec![]);
    assert!(h.app.open && h.app.page==3 && h.app.drag_hot,"文件进入感应区应立即展开置物架");
    for _ in 0..60 {h.frame(vec![]);}
    assert!(h.app.open);

    h.hovered_files.clear();
    h.cursor=None;
    let mut saw_overlay_while_closing=false;
    for _ in 0..90 {
        h.frame(vec![]);
        if h.app.open || h.app.progress()>0.02 {
            assert_eq!(h.app.page,3,"收完前不能恢复原页");
            for s in &h.shapes {
                if let egui::Shape::Text(t)=&s.shape {
                    assert!(!t.galley.text().contains("原有文件"),"收起时闪出了置物架列表");
                    saw_overlay_while_closing|=!h.app.open && t.galley.text()=="松手放进置物架";
                }
            }
        }
    }
    assert!(saw_overlay_while_closing,"虚线放置框应随岛体收回");
    assert!(!h.app.drag_hot && !h.app.open);
    assert_eq!(h.app.page,1);
    assert_eq!(h.app.shelf.items,vec![saved],"取消拖入不应保存文件");
}

#[test]
fn shelf_file_drag_reentry_preserves_the_original_page_and_release_still_saves(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.go(1,1.0);
    for _ in 0..90 {h.frame(vec![]);}
    let path=crate::model::config_dir().join("反复拖入.txt");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path,"test").unwrap();
    let point=h.file_drop_zone.unwrap().right_bottom()-Vec2::splat(12.0);
    for _ in 0..2 {
        h.cursor=Some(point);
        h.hovered_files.push(egui::HoveredFile{path:Some(path.clone()),..Default::default()});
        for _ in 0..24 {h.frame(vec![]);}
        assert!(h.app.open && h.app.page==3 && h.app.drag_hot);
        h.hovered_files.clear();
        h.cursor=None;
        for _ in 0..12 {h.frame(vec![]);}
    }
    for _ in 0..90 {h.frame(vec![]);}
    assert_eq!(h.app.page,1,"多次进出仍应回到第一次拖入前的页面");

    h.cursor=Some(point);
    h.hovered_files.push(egui::HoveredFile{path:Some(path.clone()),..Default::default()});
    for _ in 0..36 {h.frame(vec![]);}
    h.hovered_files.clear();
    h.dropped_files.push(egui::DroppedFile{path:Some(path.clone()),..Default::default()});
    for _ in 0..90 {h.frame(vec![]);}
    assert!(h.app.open && h.app.page==3);
    assert_eq!(h.app.shelf.items,vec![path.clone()]);
    assert_eq!(crate::model::Shelf::load().items,vec![path],"松手后文件应持久化到置物架");
}

#[test]
fn shelf_file_drag_exit_keeps_a_pinned_panel_open(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.go(3,1.0);
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    let pin=h.ctx.read_response(egui::Id::new(("card",true,"hold-open"))).unwrap();
    h.click(pin.rect.center());
    h.app.go(1,1.0);
    h.cursor=None;
    for _ in 0..60 {h.frame(vec![]);}
    assert!(h.app.open);
    h.hovered_files.push(egui::HoveredFile{path:Some(crate::model::config_dir().join("演示.txt")),..Default::default()});
    for _ in 0..36 {h.frame(vec![]);}
    assert!(h.app.open && h.app.page==3 && h.app.drag_hot);
    h.hovered_files.clear();
    for _ in 0..60 {h.frame(vec![]);}
    assert!(h.app.open && h.app.page==1 && !h.app.drag_hot,"取消拖入应恢复原页并保留图钉");
    assert!(!h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="松手放进置物架")));
}

#[test]
fn shelf_quick_drop_in_the_capture_zone_shows_the_added_file(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);
    let path=crate::model::config_dir().join("快速拖入.txt");std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(&path,"test").unwrap();
    h.cursor=Some(h.file_drop_zone.unwrap().right_bottom()-Vec2::splat(12.0));
    h.dropped_files.push(egui::DroppedFile{path:Some(path.clone()),..Default::default()});
    for _ in 0..90{h.frame(vec![]);}
    assert!(h.app.open && h.app.page==3);assert_eq!(h.app.shelf.items,vec![path.clone()]);assert!(path.exists());
    let mut h=H::new();for _ in 0..90{h.frame(vec![]);}
    h.cursor=Some(h.file_drop_zone.unwrap().right_bottom()-Vec2::splat(12.0));h.app.settings.visible_pages[3]=false;
    h.dropped_files.push(egui::DroppedFile{path:Some(path),..Default::default()});h.frame(vec![]);
    assert!(!h.app.open && !h.app.settings_open,"关闭置物架后应忽略拖入");
}






#[test]
fn minimal_mode_keeps_focus_running_and_returns_after_notifications(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.settings.minimal=true;h.app.pomo.running=true;h.app.pomo.remaining=120.0;
    for _ in 0..90 {h.frame(vec![]);}assert!(h.region.height()<20.0,"专注不能覆盖极简开关");
    h.app.alert("系统音量","音量 50%",5,false);
    for _ in 0..60 {h.frame(vec![]);}assert!(h.region.height()>30.0,"临时提醒仍应可见");
    for _ in 0..240 {h.frame(vec![]);}assert!(h.region.height()<20.0,"提醒结束后应回到细线");
    assert!(h.app.pomo.running&&h.app.pomo.remaining<115.0,"极简仅改变显示，计时继续运行");
}


































#[test]
fn hover_expands_with_overshoot_and_leave_collapses() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = H::new();
    h.hover(Pos2::new(WIN_W / 2.0, 12.0), 0.1);
    assert!(h.app.progress() < 0.1, "意图缓冲期内不应展开");
    let mut peak: f32 = 0.0;
    for _ in 0..60 {
        h.hover(Pos2::new(WIN_W / 2.0, 12.0), 1.0 / 60.0);
        peak = peak.max(h.app.progress());
    }
    assert!(peak > 1.01 && peak < 1.12, "展开应有轻微过冲, peak={peak}");
    assert!((h.app.progress() - 1.0).abs() < 0.02);
    h.cursor = Some(Pos2::new(WIN_W / 2.0, PH + 30.0));
    for _ in 0..40 {
        h.frame(vec![]);
    }
    assert!(h.app.progress() < 0.02, "移开后应收起");
}


#[test]
fn browser_minimal_closes_the_open_island_but_still_allows_deliberate_hover(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.settings.browser_minimal=true;
    h.app.show_page(1);h.hover(H::local(300.0,60.0),0.8);
    assert!(h.app.open);
    h.browser_at_top=Some(true);h.frame(vec![]);
    assert!(!h.app.open,"切到浏览器时不能继续保留展开状态遮挡标签栏");
    assert!(!h.hit.contains(Pos2::new(WIN_W/2.0,12.0)),"收起动画期间也应让标签栏接收点击");
    h.hover(Pos2::new(WIN_W/2.0,2.0),0.25);
    assert!(!h.app.open,"短暂碰到刘海不能误触");
    h.hover(Pos2::new(WIN_W/2.0,2.0),0.6);
    assert!(h.app.open,"仍应能主动悬停打开岛");
}

#[test]
fn minimal_hover_excludes_the_surrounding_browser_tabs() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.minimal=true;
    for _ in 0..90 {h.frame(vec![]);}
    for cursor in [
        Pos2::new(WIN_W/2.0,crate::app::BAR_H_MIN+1.0),
        Pos2::new(WIN_W/2.0+crate::app::BAR_W_MIN/2.0+2.0,2.0),
        Pos2::new(WIN_W/2.0-crate::app::BAR_W_MIN/2.0-2.0,2.0),
    ] {
        assert!(!h.hit.contains(cursor),"刘海外的标签栏应接收鼠标操作");
        h.hover(cursor,1.0);
        assert!(!h.app.open,"鼠标未触碰极简刘海时不得展开");
    }
}

#[test]
fn minimal_hover_requires_continuous_deliberate_contact() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.minimal=true;
    for _ in 0..90 {h.frame(vec![]);}
    let inside=Pos2::new(WIN_W/2.0,2.0);
    h.hover(inside,0.25);
    assert!(!h.app.open,"短暂经过极简刘海不得展开");
    h.hover(Pos2::new(WIN_W/2.0,8.0),0.1);
    h.hover(inside,0.25);
    assert!(!h.app.open,"移出刘海应重新计算停留时间");
    h.hover(inside,0.7);
    assert!(h.app.open,"在刘海内持续停留仍应展开");
    h.click(H::local(398.0,68.0));
    assert_eq!(h.app.page,2,"展开后应能正常操作面板");
}

#[test]
fn minimal_hover_does_not_catch_tabs_during_collapse() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.minimal=true;
    h.hover(Pos2::new(WIN_W/2.0,2.0),1.0);
    assert!(h.app.open);
    h.cursor=Some(Pos2::new(WIN_W/2.0,PH+30.0));
    for _ in 0..30 {
        h.frame(vec![]);
        if !h.app.open {break;}
    }
    assert!(!h.app.open);
    assert!(h.app.progress()>0.04,"检查收起动画仍在进行时的命中范围");
    let tab=Pos2::new(WIN_W/2.0,12.0);
    assert!(!h.hit.contains(tab),"收起动画不得继续拦截标签栏");
    h.hover(tab,1.0);
    assert!(!h.app.open);
}

#[test]
fn hold_open_shelf_tabs_and_all_page_exit() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for tab in [0,1] {
        let mut h=H::new();
        let pin_id=egui::Id::new(("card",true,"hold-open"));
        h.app.settings.visible_pages.fill(true);
        h.app.go(3,1.0);
        h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
        h.app.shelf_tab=tab;
        h.frame(vec![]);
        let pin=h.ctx.read_response(pin_id).expect("置物架和剪贴板都应提供图钉").rect.center();
        h.click(pin);
        let away=Pos2::new(2.0,WIN_H-2.0);
        h.app.foreground_changed(&h.ctx);
        h.browser_at_top=Some(true);
        h.app.settings.browser_minimal=true;
        h.hover(away,1.0);
        assert!(h.app.open,"切到浏览器后应保持展开");
        for page in [0,1,2,4,5,6,7,8] {
            h.app.go(page,1.0);
            h.hover(away,0.7);
            assert!(h.app.open);
            assert!(h.ctx.read_response(pin_id).is_some(),"第 {page} 页应能取消固定");
        }
        h.click(h.ctx.read_response(pin_id).unwrap().rect.center());
        h.hover(pin,0.5);
        assert!(h.app.open,"取消固定时鼠标仍在岛上");
        assert!(h.ctx.read_response(pin_id).is_none(),"其他页面淡出后不能留下图钉热区");
        h.hover(away,1.0);
        assert!(!h.app.open,"取消后恢复自动收起");
    }
}





#[test]
fn arrows_and_dots_switch_pages() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = H::new();
    h.hover(Pos2::new(WIN_W / 2.0, 12.0), 1.0);
    h.click(H::local(398.0, 68.0));
    assert_eq!(h.app.page, 2);
    h.hover(H::local(300.0, 60.0), 0.8);
    h.click(H::local(21.0, 68.0));
    assert_eq!(h.app.page, 1);
    h.hover(H::local(300.0, 60.0), 0.8);
    h.click(H::local(398.0, 68.0));
    h.hover(H::local(300.0, 60.0), 0.8);
    h.click(H::local(398.0, 68.0));
    assert_eq!(h.app.page, 3);
}

#[test]
fn wheel_navigation_reaches_every_page_in_both_directions(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.shelf.items.clear();h.app.todos.items.clear();
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    h.hover(H::local(210.0,18.0),0.3);
    for (delta,pages) in [(-40.0,[2,3,4,8,1]),(40.0,[8,4,3,2,1])] {
        for page in pages {
            h.wheel(delta);
            assert_eq!(h.app.page,page,"滚轮翻页不能被空的置物架或待办卡住");
            h.hover(H::local(210.0,18.0),0.25);
        }
    }
}

#[test]
fn wheel_from_tall_page_allows_movement_in_previous_page_bounds() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.show_page_shortcuts=false;
    let edge=H::local(70.0,165.0);
    let inner=H::local(70.0,80.0);
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    h.app.go(8,1.0);
    h.hover(edge,0.8);
    h.wheel(40.0);
    assert_eq!(h.app.page,4);
    for _ in 0..90 {h.frame(vec![]);}
    assert!(h.app.open && !h.hit.contains(edge),"原位保留翻页意图，岛外仍可点击桌面");
    assert!((h.app.h.pos-PH).abs()<0.1 && (h.hit.height()-PH-2.0).abs()<0.1,"矮页应使用自身高度");
    h.hover(H::local(180.0,174.0),0.4);
    assert!(h.app.open,"在上一页范围内向侧下方移动仍可阅读");
    h.hover(H::local(220.0,PH-1.0),0.4);
    h.hover(H::local(250.0,157.0),0.4);
    assert!(h.app.open,"轻微穿过新页边缘再移回来不应误收起");
    h.wheel(40.0);
    assert_eq!(h.app.page,3);
    h.hover(inner,0.5);
    assert!(h.app.open && !h.hit.contains(edge),"移入新页后恢复该页高度");
    h.hover(edge,0.5);
    assert!(!h.app.open,"重新移出新页应正常收起");

    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    h.app.go(8,1.0);
    h.hover(H::local(70.0,PH-1.0),0.8);
    h.wheel(40.0);
    h.hover(edge,0.4);
    assert!(h.app.open,"翻页时光标刚好在新边缘内，也能顺势移到旧页范围");
    h.hover(H::local(-25.0,165.0),0.5);
    assert!(!h.app.open,"直接移出原岛体也应正常收起");
}

#[test]
fn side_docks_keep_both_edges_when_turning_to_a_shorter_page() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for dock in [crate::model::Dock::Left,crate::model::Dock::Right] {
        let mut h=H::new();
        h.app.settings.show_page_shortcuts=false;
        h.app.settings.dock=dock;
        let x=if dock==crate::model::Dock::Left {1.0}else{crate::side_dock::SIDE_W-1.0};
        let resident=Pos2::new(x,crate::side_dock::SIDE_H/2.0);
        for _ in 0..60 {side_frame(&mut h,Some(resident));}
        assert!(h.app.open);
        for sign in [-1.0,1.0] {
            h.app.go(8,1.0);
            let tall=side_frame(&mut h,Some(resident)).hit;
            let edge=tall.center()+Vec2::new(0.0,sign*83.0);
            for _ in 0..45 {side_frame(&mut h,Some(edge));}
            side_frame_events(&mut h,Some(edge),vec![egui::Event::MouseWheel {
                unit:egui::MouseWheelUnit::Point,delta:Vec2::new(0.0,40.0),modifiers:Default::default(),
            }]);
            assert_eq!(h.app.page,4);
            let moved=edge+Vec2::new(65.0,4.0*sign);
            for _ in 0..45 {side_frame(&mut h,Some(moved));}
            let out=side_frame(&mut h,Some(moved));
            assert!(out.expanded && !out.hit.contains(moved) && out.page_scroll_area.is_some_and(|r|r.contains(moved)),
                "{dock:?} 原页上下边缘的空白处应可移动并续滚");
            assert!((h.app.h.pos-PH).abs()<0.1);
            let out=side_frame(&mut h,Some(tall.center()));
            assert!(out.expanded && !out.hit.contains(edge),"{dock:?} 移入矮页后应恢复高度");
        }
    }
}

#[test]
fn wheel_scrolls_lists_only_inside_content_and_respects_edges(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.todos.items=(0..4).map(|i|crate::model::TodoItem{text:format!("任务 {i}"),done_on:None,due:None,reminded:false,..Default::default()}).collect();
    h.app.shelf.items=(0..6).map(|i|std::path::PathBuf::from(format!("scroll-test-{i}.txt"))).collect();
    for page in [3,4] {
        h.app.show_page(page);h.hover(H::local(300.0,60.0),1.0);
        h.wheel(-40.0);
        assert_eq!(h.app.page,page,"有更多内容时应先滚动列表");
        assert_eq!(if page==3 {h.app.shelf_first}else{h.app.todo_first},1);
        if page==3 {
            for _ in 0..10 {h.frame(vec![]);assert!(h.interactive_transition);}
            for _ in 0..90 {h.frame(vec![]);}
            assert!(!h.interactive_transition,"置物架停稳后应降帧");
        }
        h.hover(H::local(300.0,60.0),0.25);h.wheel(-40.0);
        assert_eq!(h.app.page,page,"内容溢出时滚到底也应留在本页");
        h.app.show_page(page);h.hover(H::local(210.0,18.0),0.3);h.wheel(-40.0);
        assert_eq!(h.app.page,if page==4{8}else{page+1},"顶栏滚轮不应被内容列表拦截");
    }
}

#[test]
fn wheel_overflowing_shelf_stays_at_both_edges(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    for count in [6,9]{
        h.app.shelf.items=(0..count).map(|i|std::path::PathBuf::from(format!("scroll-test-{i}.txt"))).collect();
        h.app.shelf_first=0;h.app.show_page(3);
        let last=count-5;
        for (delta,first) in [(40.0_f32,0),(-40.0,1),(-400.0,last),(-40.0,last),(40.0,last-1),(400.0,0),(40.0,0)]{
            h.hover(H::local(300.0,60.0),0.7);
            for _ in 0..(delta.abs() as usize/40){h.wheel(delta.signum()*40.0);}
            assert_eq!(h.app.page,3,"置物架溢出后连续滚动或停顿再滚都应留在本页，条数 {count}");
            assert_eq!(h.app.shelf_first,first);
        }
        for (delta,page) in [(-40.0,4),(40.0,2)]{
            h.hover(H::local(300.0,60.0),0.7);
            h.click(H::local(if delta<0.0 {398.0}else{21.0},68.0));
            assert_eq!(h.app.page,page,"置物架溢出后仍应允许箭头切页");
            h.app.show_page(3);h.hover(H::local(210.0,18.0),0.3);h.wheel(delta);
            assert_eq!(h.app.page,page,"置物架顶栏仍应允许滚轮翻页");
            h.app.show_page(3);
        }
    }
}

#[test]
fn wheel_overflowing_todos_stay_at_both_edges(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    for count in [4,8]{
        h.app.todos.items=(0..count).map(|i|crate::model::TodoItem{text:format!("任务 {i}"),..Default::default()}).collect();
        h.app.todo_first=0;h.app.show_page(4);
        let last=count-3;
        for (delta,first) in [(40.0_f32,0),(-40.0,1),(-400.0,last),(-40.0,last),(40.0,last-1),(400.0,0),(40.0,0)]{
            h.hover(H::local(300.0,60.0),0.3);
            for _ in 0..(delta.abs() as usize/40){h.wheel(delta.signum()*40.0);}
            assert_eq!(h.app.page,4,"待办溢出后滚动到首尾都不应翻页，条数 {count}");
            assert_eq!(h.app.todo_first,first);
        }
        for (delta,page) in [(-40.0,8),(40.0,3)]{
            h.app.show_page(4);h.hover(H::local(100.0,60.0),0.3);h.wheel(delta);
            assert_eq!(h.app.page,page,"待办列表外仍应允许滚轮翻页");
        }
    }
}

#[test]
fn wheel_clipboard_stays_at_both_edges() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.show_page(3);
    h.app.shelf_tab=1;
    for count in [crate::pages::CLIP_ROWS+1] {
        h.app.clipboard.lock().items.clear();
        for i in 0..count { h.app.clipboard.lock().push(&i.to_string(), "09:00"); }
        let last=count.saturating_sub(crate::pages::CLIP_ROWS);
        for (delta, first) in [(40.0,0),(-40.0,last),(-40.0,last),(40.0,0),(40.0,0)] {
            h.hover(H::local(300.0,60.0),0.4);
            h.wheel(delta);
            assert_eq!(h.app.page,3);
            assert_eq!(h.app.clip_first,first);
        }
        for (delta,page) in [(-40.0,4),(40.0,2)]{
            h.app.show_page(3);h.hover(H::local(210.0,18.0),0.4);h.wheel(delta);
            assert_eq!(h.app.page,page,"剪贴板列表外仍应允许滚轮翻页");
        }
    }
}
#[test]
fn hover_morph_keeps_interactive_frames_after_input_stops() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    for cursor in [Some(Pos2::new(WIN_W/2.0,12.0)),None] {
        h.cursor=cursor;
        let mut moving=0;
        for _ in 0..120 {
            let before=h.app.progress();
            h.frame(vec![]);
            if (h.app.progress()-before).abs()>0.005 {
                assert!(h.interactive_transition,"岛体仍在运动时应逐帧显示");
                moving+=1;
            }
        }
        assert!(moving>4);
        assert!(!h.interactive_transition,"停稳后恢复闲置刷新");
    }
}

#[test]
fn collapse_readback_covers_the_returning_pet_then_goes_idle() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.set_pet(true);
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.5);
    assert!(h.app.debug_state().1);
    h.cursor=None;
    for _ in 0..60 {
        h.frame(vec![]);
        if !h.app.debug_state().1 {break;}
    }
    assert!(!h.app.debug_state().1);
    for _ in 0..36 {h.frame(vec![]);}
    assert!(h.interactive_transition,"收起后宠物还在淡入，读回应保持交互节奏");
    for _ in 0..90 {h.frame(vec![]);}
    assert!(!h.interactive_transition,"宠物淡入结束后应恢复闲置节奏");
}


#[test]
fn page_turn_fades_old_content_in_place_and_slides_new_content_without_rebound() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.pet=false;
    // 健康页的标题随提醒状态变化（待活动 / 等待提醒 / 已关闭 / 分钟后提醒），按候选文案匹配。
    let labels: [&[&str];3] = [&[], &["专注时间"], &["喝一口"]];
    let body_text=|h:&H,page:usize| h.shapes.iter().find_map(|shape|match &shape.shape {
        egui::Shape::Text(text) if labels[page].contains(&text.galley.text()) && text.fallback_color.a()>2
            =>Some((text.pos.x,text.fallback_color.a())),
        _=>None,
    });
    h.hover(H::local(210.0,18.0),1.2);

    for (wheel,target,direction) in [(false,2,1.0),(false,1,-1.0),(true,2,1.0),(true,1,-1.0)] {
        let previous=h.app.page();
        let (old_x,old_alpha)=body_text(&h,previous).expect("切页前应能看到正文");
        if wheel {
            h.hover(H::local(210.0,18.0),0.05);
            h.wheel(-40.0*direction);
        } else {
            h.click(H::local(if direction>0.0 {398.0}else{21.0},68.0));
        }
        assert_eq!(h.app.page(),target);

        let mut saw_fade=false;
        let mut incoming=Vec::new();
        for _ in 0..40 {
            if let Some((x,alpha))=body_text(&h,previous) {
                assert!((x-old_x).abs()<0.05,"旧页正文应在原位淡出：{old_x} → {x}");
                saw_fade|=alpha<old_alpha;
            }
            if let Some((x,_))=body_text(&h,target) {incoming.push(x);}
            h.frame(vec![]);
        }
        assert!(saw_fade && body_text(&h,previous).is_none(),"旧页应先淡出再消失");
        let (target_x,alpha)=body_text(&h,target).expect("翻页完成后应能看到新页正文");
        assert!(alpha==255 && incoming.len()>6);
        let distance=|x:f32| direction*(x-target_x);
        assert!(distance(incoming[0])>5.0,"新页应从翻页方向对应的一侧滑入");
        for positions in incoming.windows(2) {
            let (before,after)=(distance(positions[0]),distance(positions[1]));
            assert!(after>=-0.001 && after<=before+0.001,"新页不应越过终点或向回移动：{before} → {after}");
        }
    }

    // 滚轮可以在正文滑入、标题尚未出现时反向切回。
    h.wheel(-40.0);
    for _ in 0..4 {h.frame(vec![]);}
    assert_eq!(h.app.page(),2);
    let (sliding_x,_)=body_text(&h,2).expect("反向前应已出现健康正文");
    h.wheel(40.0);
    assert_eq!(h.app.page(),1);
    let (fading_x,_)=body_text(&h,2).expect("反向后应保留健康正文淡出");
    assert!((fading_x-sliding_x).abs()<5.0,"淡出位置应接续上一个滑入帧");
    for _ in 0..12 {
        if let Some((x,_))=body_text(&h,2) {
            assert!((x-fading_x).abs()<0.05,"中途反向后旧正文应停在当前位置");
        }
        assert!(!h.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text)
            if text.galley.text()=="健康" && text.fallback_color.a()>2)),"尚未出现的旧标题不应闪现");
        h.frame(vec![]);
    }
    assert!(body_text(&h,2).is_none(),"中途反向也应完成旧页淡出");
}

#[test]
fn wheel_page_transition_keeps_interactive_frames_after_input_stops() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    h.hover(H::local(210.0,18.0),0.3);
    assert!(!h.interactive_transition);
    h.wheel(-40.0);
    assert_eq!(h.app.page,2);
    assert!(h.interactive_transition);
    for _ in 0..18 {
        h.frame(vec![]);
        assert!(h.interactive_transition,"松开滚轮后，仍然可见的翻页动画应继续逐帧显示");
    }
    for _ in 0..24 {h.frame(vec![]);}
    assert!(!h.interactive_transition,"翻页结束后应恢复原刷新策略");
}

#[test]
fn wheel_handles_small_deltas_without_skipping_pages_on_bursts(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);h.hover(H::local(210.0,18.0),0.3);
    for _ in 0..16 {h.wheel(-0.5);}
    assert_eq!(h.app.page,2,"高精度滚轮的小幅输入应能累积成一次翻页");
    h.hover(H::local(210.0,18.0),0.3);
    for _ in 0..5 {h.wheel(-40.0);}
    assert_eq!(h.app.page,3,"一次连续滚动不能瞬间跳过多个页面");
}






#[test]
#[ignore = "按需导出常驻组件排布与补位动画"]
fn resident_layout_gallery(){
    fn demo(battery:bool,_accounts:usize,_music:bool,focus:bool,pet:bool)->H{
        let mut h=H::new();h.app.settings.show_battery=battery;h.app.settings.pet=pet;h.power=(Some(80),true);
        if focus {h.app.pomo.running=true;h.app.pomo.remaining=1283.0;}
        if pet {h.app.pet.react(crate::extras::PetState::Happy);}
        h
    }
    fn capture(h:&mut H,raster:&mut crate::preview_raster::Raster,t:f64,draw:bool)->Vec<u8>{
        let env=Env{t,clock:"09:41:00".into(),today:"2026-10-31".into(),date:"10月31日 周六".into(),battery:h.power.0,charging:h.power.1,..Default::default()};
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(WIN_W,WIN_H))),time:Some(t),
            viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(2.0),..Default::default()})).collect(),..Default::default()};
        let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{let win=ui.max_rect();h.app.frame(ui,win,&env);});});
        raster.update_textures(&output.textures_delta);let mut rgba=Vec::new();
        if draw {raster.buf.fill([0.09,0.11,0.18,1.0]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);raster.to_rgba8(&mut rgba);}
        raster.free_textures(&output.textures_delta);rgba
    }

    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_RESIDENT_PREVIEW_DIR").expect("需指定预览目录"));std::fs::create_dir_all(&dest).unwrap();
    let scenes=[(true,1,false,false,false),(false,1,false,false,false),(true,3,false,false,false),(false,3,true,false,false),
        (true,3,true,false,false),(false,3,true,true,false),(false,5,false,false,false),(false,2,false,false,true)];
    for (i,(battery,accounts,music,focus,pet)) in scenes.into_iter().enumerate(){
        let mut h=demo(battery,accounts,music,focus,pet);let mut raster=crate::preview_raster::Raster::new(1280,380);
        for frame in 0..90 {
            let rgba=capture(&mut h,&mut raster,frame as f64/60.0,frame==89);
            if frame==89 {image::save_buffer(dest.join(format!("layout-{i}.png")),&rgba,1280,380,image::ColorType::Rgba8).unwrap();}
        }
    }
    let mut h=demo(true,3,true,false,false);let mut raster=crate::preview_raster::Raster::new(1280,380);
    for frame in 0..240 {
        if frame==60 {h.app.settings.show_battery=false;}if frame==180 {h.app.settings.show_battery=true;}
        let rgba=capture(&mut h,&mut raster,frame as f64/60.0,frame%3==0);
        if frame%3==0 {
            let image=image::RgbaImage::from_raw(1280,380,rgba).unwrap();
            image::imageops::crop_imm(&image,200,0,880,80).to_image().save(dest.join(format!("frame-{frame:03}.png"))).unwrap();
        }
    }
    eprintln!("常驻组件预览：{}",dest.display());
}


















#[test]
fn battery_module_hides_power_in_collapsed_and_expanded_headers() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for expanded in [false,true] {
        let mut h=H::new();
        h.app.settings.pet=false;
        h.app.settings.show_battery=false;
        if expanded {h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);} else {h.frame(vec![]);}
        let baseline=h.shapes.clone();
        h.power=(Some(80),true);
        h.t-=1.0/60.0;h.frame(vec![]);
        assert!(h.shapes==baseline,"关闭电量后，本机电池状态不应影响界面布局");
        h.app.settings.show_battery=true;
        h.t-=1.0/60.0;h.frame(vec![]);
        assert!(h.shapes!=baseline,"开启后应显示电量");
        h.power=(None,true);
        h.t-=1.0/60.0;h.frame(vec![]);
        assert!(h.shapes==baseline,"没有电池的设备应自动隐藏电量并释放占位");
    }
}

#[test]
fn battery_module_can_disable_only_the_power_warning() {
    use crate::extras::PetState;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.show_battery=false;
    h.power=(Some(10),false);h.frame(vec![]);
    assert_eq!(h.app.pet_state(),PetState::Awake);
    h.network=Some(false);h.frame(vec![]);
    assert_eq!(h.app.pet_state(),PetState::Worried);
}

#[test]
fn page_visibility_skips_hidden_pages_in_every_navigation_control() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    for page in [1,3,4,6,7,9,10] {h.app.set_page_visible(page,false);}
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    for expected in [8,2,8] {
        h.click(H::local(398.0,68.0));
        assert_eq!(h.app.page,expected);
        h.hover(H::local(300.0,60.0),0.7);
    }
    let dot=h.ctx.read_response(egui::Id::new(("card",true,("dot",2usize)))).unwrap();
    h.click(dot.interact_rect.center());
    assert_eq!(h.app.page,2,"底部圆点应仅包含启用的页面");
    h.hover(H::local(300.0,60.0),0.7);
    h.frame(vec![egui::Event::Key{key:egui::Key::ArrowRight,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
    assert_eq!(h.app.page,8);
    h.frame(vec![egui::Event::Key{key:egui::Key::ArrowRight,physical_key:None,pressed:false,repeat:false,modifiers:Default::default()}]);
    h.hover(H::local(300.0,60.0),0.7);
    h.frame(vec![egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:Vec2::new(0.0,40.0),modifiers:Default::default()}]);
    assert_eq!(h.app.page,2,"滚轮应跳过隐藏页面");
    h.click(H::local(21.0,68.0));
    assert_eq!(h.app.page,8);
}

#[test]
fn page_visibility_moves_away_from_hidden_page_and_keeps_one_page() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.go(4,1.0);
    h.app.adding=true;
    h.app.set_page_visible(4,false);
    assert_eq!(h.app.page,1);
    assert!(!h.app.is_editing());
    for page in [1,3,8,9] {h.app.set_page_visible(page,false);}
    assert_eq!(h.app.page,2);
    h.app.set_page_visible(2,false);
    assert!(h.app.settings.visible_pages[2],"不能关闭最后一个页面");
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    h.click(H::local(398.0,68.0));
    h.click(H::local(21.0,68.0));
    assert_eq!(h.app.page,2);
    h.app.set_page_visible(9,true);
    h.hover(H::local(300.0,60.0),0.7);
    h.click(H::local(398.0,68.0));
    assert_eq!(h.app.page,9,"重新开启的页面应恢复导航");
}

#[test]
fn page_visibility_does_not_reopen_hidden_pages_through_shortcuts_or_drops() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.set_page_visible(5,false);
    h.app.show_page(5);
    assert_eq!(h.app.page,1);
    assert!(h.app.settings_open);
    assert_eq!(h.app.settings_tab,7,"隐藏页面的入口应进入对应设置");
    h.app.set_page_visible(3,false);
    let file=crate::model::config_dir().join("shelf-hidden.txt");
    std::fs::create_dir_all(crate::model::config_dir()).unwrap();
    std::fs::write(&file,"test").unwrap();
    h.app.on_drop(vec![file]);
    assert_eq!(h.app.page,1);
    assert!(h.app.shelf.items.is_empty());
}












#[test]
fn empty_todo_draft_can_be_cancelled_by_clicking_elsewhere(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for text in ["","   "]{
        let mut h=H::new();let mut r=feature_raster();h.app.todos.items=vec![crate::model::TodoItem{text:"保留任务".into(),due:None,done_on:None,reminded:false,..Default::default()}];h.app.todos.save();
        h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(70.0,109.0));
        assert!(h.app.adding,"点击添加后应保留空白输入框");
        if !text.is_empty(){h.frame(vec![egui::Event::Text(text.into())]);}
        h.cmds.clear();h.click(H::local(65.0,60.0));
        assert!(!h.app.is_editing(),"空白草稿点击岛内其他位置应退出");
        assert!(h.app.buf.is_empty());assert_eq!(h.app.todos.items.len(),1);assert_eq!(h.app.todos.items[0].text,"保留任务");
        assert_eq!(crate::model::TodoList::load().items.len(),1,"不能保存空白待办");
        h.hover(H::local(65.0,60.0),0.3);
        assert!(!h.cmds.iter().any(|cmd|matches!(cmd,crate::app::Cmd::Focus)),"取消后不能反复激活窗口");
    }
}


#[test]
fn todo_ime_first_backspace_keeps_committed_text(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(70.0,109.0));
    h.frame(vec![egui::Event::Text("保留文字".into())]);
    h.frame(vec![egui::Event::Key{key:egui::Key::Backspace,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()},egui::Event::Ime(egui::ImeEvent::Enabled)]);
    assert_eq!(h.app.buf,"保留文字","开始组合输入的同一帧，退格只能交给输入法");
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Disabled)]);
    h.frame(vec![egui::Event::Key{key:egui::Key::Backspace,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
    assert_eq!(h.app.buf,"保留文","结束组合输入后普通退格仍应正常删除");
}

#[test]
fn todo_ime_focus_change_preserves_empty_draft(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(70.0,109.0));h.cmds.clear();
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled),egui::Event::WindowFocused(false)]);
    assert!(h.app.adding,"输入法候选窗口造成的失焦不能取消空白待办");
    assert!(h.app.open);
    assert!(!h.cmds.iter().any(|cmd|matches!(cmd,crate::app::Cmd::Focus)),"不能和输入法窗口反复争抢焦点");
    h.app.foreground_changed(&h.ctx);h.frame(vec![]);
    assert!(h.app.adding,"输入法组合输入期间的前台切换应保留草稿");
    h.frame(vec![egui::Event::WindowFocused(true),egui::Event::Ime(egui::ImeEvent::Preedit("zhong".into()))]);
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("中文".into())),egui::Event::Ime(egui::ImeEvent::Disabled)]);
    assert_eq!(h.app.buf,"中文");assert!(h.app.todos.items.is_empty());
}

#[test]
fn todo_ime_confirmation_does_not_submit_or_cancel_the_draft(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for key in [egui::Key::Enter,egui::Key::Escape]{
        let mut h=H::new();h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(70.0,109.0));
        h.frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
        h.frame(vec![egui::Event::Key{key,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()},egui::Event::Ime(egui::ImeEvent::Commit("中文".into())),egui::Event::Ime(egui::ImeEvent::Disabled)]);
        assert!(h.app.adding,"输入法确认或取消候选不能同时结束待办编辑");
        assert_eq!(h.app.buf,"中文","输入法确认后应保留正文，按键 {key:?}");assert!(h.app.todos.items.is_empty());
        h.frame(vec![egui::Event::Key{key,physical_key:None,pressed:false,repeat:false,modifiers:Default::default()}]);
        h.frame(vec![egui::Event::Key{key:egui::Key::Enter,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
        assert_eq!(h.app.todos.items.len(),1,"退出组合输入后回车应正常保存");assert_eq!(h.app.todos.items[0].text,"中文");
    }
}

#[test]
fn todo_ime_candidate_closing_does_not_discard_the_empty_draft(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(70.0,109.0));
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Disabled),egui::Event::WindowFocused(false)]);
    assert!(h.app.adding,"关闭输入法候选窗口的同一帧不能取消空白草稿");
    h.frame(vec![egui::Event::WindowFocused(true)]);h.app.foreground_changed(&h.ctx);h.frame(vec![]);
    assert!(!h.app.adding,"之后真正切换应用时仍应取消空白草稿");
}

#[test]
fn todo_ime_event_variants_preserve_mixed_text_and_resume_plain_input(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let cases=vec![
        ("无显式开始事件",vec![vec![egui::ImeEvent::Preedit("ni".into())],vec![egui::ImeEvent::Commit("你".into()),egui::ImeEvent::Disabled]],"你"),
        ("一次提交多个字符",vec![vec![egui::ImeEvent::Enabled,egui::ImeEvent::Preedit("にほん".into())],vec![egui::ImeEvent::Commit("日本".into()),egui::ImeEvent::Disabled]],"日本"),
        ("同帧提交后继续组合",vec![vec![egui::ImeEvent::Enabled,egui::ImeEvent::Preedit("하".into())],vec![egui::ImeEvent::Commit("한".into()),egui::ImeEvent::Disabled,egui::ImeEvent::Enabled,egui::ImeEvent::Preedit("그".into())],vec![egui::ImeEvent::Commit("글".into()),egui::ImeEvent::Disabled]],"한글"),
        ("取消候选",vec![vec![egui::ImeEvent::Enabled,egui::ImeEvent::Preedit("pin".into())],vec![egui::ImeEvent::Preedit(String::new()),egui::ImeEvent::Disabled]],""),
    ];
    for (name,frames,expected) in cases {
        let mut h=H::new();h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(70.0,109.0));
        h.frame(vec![egui::Event::Text("中文 ABC ".into())]);
        for events in frames {h.frame(events.into_iter().map(egui::Event::Ime).collect());}
        assert_eq!(h.app.buf,format!("中文 ABC {expected}"),"事件模式：{name}");
        assert!(h.app.adding);assert!(h.app.todos.items.is_empty());
        h.frame(vec![egui::Event::Text("x".into())]);
        h.frame(vec![egui::Event::Key{key:egui::Key::Backspace,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
        assert_eq!(h.app.buf,format!("中文 ABC {expected}"),"组合输入结束后应恢复普通退格：{name}");
        h.frame(vec![egui::Event::Key{key:egui::Key::Enter,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
        assert_eq!(h.app.todos.items.len(),1,"组合输入结束后应能保存：{name}");
    }
}

#[test]
fn blank_todo_draft_closes_when_window_focus_is_lost(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(70.0,109.0));
    h.cmds.clear();h.frame(vec![egui::Event::WindowFocused(false)]);
    assert!(!h.app.is_editing(),"点击其他软件后空白草稿应自动退出");
    assert!(h.app.todos.items.is_empty());
    assert!(!h.cmds.iter().any(|cmd|matches!(cmd,crate::app::Cmd::Focus)),"失去焦点时不能把窗口再次激活");
}

#[test]
fn todo_input_requests_window_focus_once_and_preserves_typed_drafts(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.cmds.clear();h.click(H::local(70.0,109.0));
    assert_eq!(h.cmds.iter().filter(|cmd|matches!(cmd,crate::app::Cmd::Focus)).count(),1,"新建时只应请求一次窗口焦点");
    h.click(H::local(280.0,46.0));assert!(h.app.adding,"点击输入框不能取消草稿");
    h.frame(vec![egui::Event::Text("尚未保存的内容".into())]);h.cmds.clear();
    h.click(H::local(65.0,60.0));h.frame(vec![egui::Event::WindowFocused(false)]);
    for _ in 0..20{h.frame(vec![]);}
    assert!(h.app.adding);assert_eq!(h.app.buf,"尚未保存的内容");assert!(h.app.todos.items.is_empty());
    assert!(!h.cmds.iter().any(|cmd|matches!(cmd,crate::app::Cmd::Focus)),"已有内容失焦后应保留草稿，不能抢回焦点");
}

#[test]
fn switching_apps_cancels_only_empty_new_todos(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for text in ["","   ","保留未提交的内容"]{
        let mut h=H::new();h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(70.0,109.0));
        if !text.is_empty(){h.frame(vec![egui::Event::Text(text.into())]);}
        h.cmds.clear();h.app.foreground_changed(&h.ctx);h.frame(vec![]);
        assert_eq!(h.app.adding,!text.trim().is_empty());
        assert_eq!(h.app.buf,if text.trim().is_empty(){""}else{text});
        assert!(h.app.todos.items.is_empty());
        assert!(!h.cmds.iter().any(|cmd|matches!(cmd,crate::app::Cmd::Focus)));
    }
}

#[test]
fn drink_adds_water_and_todo_can_be_added() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = H::new();
    h.hover(Pos2::new(WIN_W / 2.0, 12.0), 1.0);
    h.app.go(2, 1.0);
    h.hover(H::local(300.0, 60.0), 0.8);
    h.click(H::local(70.0, 110.0));
    assert_eq!(h.app.water.drunk_ml, 250);
    assert_eq!(h.app.water.hours[9], 250);

    h.app.go(4, 1.0);
    h.hover(H::local(300.0, 60.0), 0.8);
    h.click(H::local(70.0, 109.0));
    assert!(h.app.adding);
    h.hover(H::local(300.0, 60.0), 0.4);
    h.frame(vec![egui::Event::Text("写周报".into())]);
    h.frame(vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    h.hover(H::local(300.0, 60.0), 0.1);
    assert_eq!(h.app.todos.items.first().map(|t| t.text.as_str()), Some("写周报"));
    assert!(!h.app.adding);
}

#[test]
fn timer_counts_elapsed_time_after_sleep() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = H::new();
    h.frame(vec![]);
    h.app.pomo.running = true;
    h.app.pomo.remaining = 120.0;
    h.t += 60.0;
    h.frame(vec![]);
    assert!(h.app.pomo.remaining < 61.0, "挂起恢复后必须补算实际流逝时间");
}

#[test]
fn shelf_remove_keeps_original_file() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let file=crate::model::config_dir().join("保留文件.txt");std::fs::create_dir_all(file.parent().unwrap()).unwrap();std::fs::write(&file,"内容").unwrap();
    h.app.shelf.add(&file).unwrap();h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);h.app.go(3,1.0);h.hover(H::local(300.0,60.0),0.8);
    h.click(H::local(233.0,39.0));assert!(h.app.shelf.items.is_empty());assert!(file.exists());assert!(crate::model::Shelf::load().items.is_empty());
}

#[test]
fn shelf_duplicate_drop_reports_existing_files_and_preserves_the_current_page() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let dir=crate::model::config_dir();std::fs::create_dir_all(&dir).unwrap();
    let files:Vec<_>=(0..7).map(|i|{let path=dir.join(format!("文件-{i}.txt"));std::fs::write(&path,"内容").unwrap();path}).collect();
    h.app.on_drop(files.clone());h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);h.app.shelf_first=0;
    h.app.on_drop(vec![files[0].clone()]);h.hover(H::local(300.0,60.0),0.3);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="该文件已在置物架中")));
    h.app.on_drop(vec![files[0].clone(),files[1].clone()]);h.hover(H::local(300.0,60.0),0.3);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="这些文件已在置物架中")));
    assert_eq!(h.app.shelf_first,0,"重复添加不应将列表跳到最后");
    assert_eq!(h.app.shelf.items,files);assert_eq!(crate::model::Shelf::load().items,files);
}

#[test]
fn shelf_drop_distinguishes_new_duplicate_and_missing_files() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let dir=crate::model::config_dir();std::fs::create_dir_all(&dir).unwrap();
    let old=dir.join("已添加.txt");let new=dir.join("新文件.txt");let missing=dir.join("已移动.txt");
    for file in [&old,&new]{std::fs::write(file,"内容").unwrap();}
    h.app.on_drop(vec![old.clone()]);h.app.on_drop(vec![old.clone(),new.clone(),new.clone(),missing.clone()]);
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="已添加 1 个文件，2 个已存在，1 个无法添加")));
    assert_eq!(h.app.shelf.items,vec![old.clone(),new]);
    std::fs::remove_file(&old).unwrap();h.app.on_drop(vec![old]);h.hover(H::local(300.0,60.0),0.3);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="文件不存在或无法访问")));
}
#[test]
fn reminders_are_visible_while_collapsed_and_can_be_disabled(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);
    h.t+=2701.0;h.frame(vec![]);assert!(h.app.notice.is_some());assert!(h.cmds.iter().any(|c|matches!(c,crate::app::Cmd::Notify(t,..)if t.contains("喝水"))));
    h.app.water.remind=false;h.app.settings.sit_remind=false;h.app.notice=None;h.cmds.clear();h.t+=2800.0;h.frame(vec![]);assert!(h.app.notice.is_none());
}
#[test]
fn water_view_transition_keeps_repainting_after_the_click() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = H::new();
    h.app.show_page(crate::app::WATER_PAGE);
    let toggle = H::local(292.0, 78.0);
    h.hover(toggle, 2.0);
    assert!(!h.animating);

    for target in [1, 0] {
        h.click(toggle);
        assert_eq!(h.app.water_view, target);
        assert!(h.animating, "点击松开当帧就应继续安排重绘");
        for _ in 0..12 {
            h.frame(vec![]);
            assert!(h.animating, "没有后续输入时也应持续重绘过渡");
        }
        for _ in 0..48 { h.frame(vec![]); }
        assert!(h.app.water_view_t.settled(1.0));
        assert!(!h.animating, "过渡结束后应恢复静态刷新");
    }
}

#[test]
fn water_serving_and_undo_are_consistent(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);h.app.go(2,1.0);h.hover(H::local(300.0,60.0),0.8);
    h.app.settings.serving_ml=350;h.click(H::local(70.0,110.0));assert_eq!(h.app.water.drunk_ml,350);assert_eq!(h.app.water.hours[9],350);
    h.click(H::local(191.0,110.0));assert_eq!(h.app.water.drunk_ml,0);assert_eq!(h.app.water.hours[9],0);
}
#[test]
fn mascot_rest_and_focus_have_distinct_states(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.opts.pet_idle=45.0;h.frame(vec![]);
    assert_eq!(h.app.pet_state(),crate::extras::PetState::Awake);
    h.t+=50.0;h.frame(vec![]);assert_eq!(h.app.pet_state(),crate::extras::PetState::Dozing);
    h.t+=140.0;h.frame(vec![]);assert_eq!(h.app.pet_state(),crate::extras::PetState::Sleeping);
    h.app.pomo.running=true;assert_eq!(h.app.pet_state(),crate::extras::PetState::Focused);
}






#[test]
fn selecting_permanent_mascot_or_minimal_applies_the_selected_mode(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.set_minimal(true);for _ in 0..90 {h.frame(vec![]);}
    assert!(h.app.settings.minimal && !h.app.settings.pet && h.region.height()<20.0);
    h.app.set_pet(true);for _ in 0..90 {h.frame(vec![]);}
    assert!(h.app.settings.pet && !h.app.settings.minimal && h.region.height()>30.0);
    let saved=crate::model::Settings::load();assert!(saved.pet && !saved.minimal);
}
#[test]
fn mascot_wakes_before_returning_to_idle(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.opts.pet_idle=45.0;
    h.frame(vec![]);h.t+=190.0;for _ in 0..60 {h.frame(vec![]);}
    assert_eq!(format!("{:?}",h.app.pet_state()),"Sleeping");
    h.cursor=Some(Pos2::new(550.0,100.0));h.frame(vec![egui::Event::PointerMoved(h.cursor.unwrap())]);
    assert_eq!(format!("{:?}",h.app.pet_state()),"Waking");
    for _ in 0..180 {h.frame(vec![]);}
    assert_eq!(format!("{:?}",h.app.pet_state()),"Awake");
}
#[test]
fn mascot_keeps_task_celebration_until_panel_closes(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);
    h.app.todos.items.push(crate::model::TodoItem{text:"完成测试任务".into(),done_on:None,due:None,reminded:false,..Default::default()});
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);h.app.go(4,1.0);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(250.0,47.0));
    assert!(h.app.todos.items[0].done_on.is_some());
    h.hover(H::local(300.0,60.0),4.0);assert_eq!(format!("{:?}",h.app.pet_state()),"Happy");
    h.cursor=None;for _ in 0..240 {h.frame(vec![]);}
    assert_eq!(format!("{:?}",h.app.pet_state()),"Awake");
}
#[test]
fn mascot_drink_nods_and_focus_completion_celebrates(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);h.app.drink_water();
    assert_eq!(format!("{:?}",h.app.pet_state()),"Satisfied");
    for _ in 0..240 {h.frame(vec![]);}
    h.app.pomo.running=true;h.app.pomo.remaining=0.01;h.frame(vec![]);
    assert_eq!(format!("{:?}",h.app.pet_state()),"Happy");
}
#[test]
fn mascot_power_and_network_recover_without_stale_warning(){
    use crate::extras::PetState;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.power=(Some(15),false);h.frame(vec![]);assert_eq!(h.app.pet_state(),PetState::Worried);
    h.power.1=true;h.frame(vec![]);assert_eq!(h.app.pet_state(),PetState::Awake);
    h.network=Some(false);h.frame(vec![]);assert_eq!(h.app.pet_state(),PetState::Worried);
    h.network=Some(true);h.frame(vec![]);assert_eq!(h.app.pet_state(),PetState::Awake);
    h.power=(None,false);h.network=None;h.frame(vec![]);assert_eq!(h.app.pet_state(),PetState::Awake);
}
#[test]
fn mascot_system_idle_wakes_once_then_follows_pointer(){
    use crate::extras::PetState;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.opts.pet_idle=45.0;
    h.system_idle=Some(200.0);h.frame(vec![]);assert_eq!(h.app.pet_state(),PetState::Sleeping);
    h.system_idle=Some(0.0);h.frame(vec![]);assert_eq!(h.app.pet_state(),PetState::Waking);
    for _ in 0..180 {h.frame(vec![]);}assert_eq!(h.app.pet_state(),PetState::Awake);
    h.cursor=Some(Pos2::new(WIN_W/2.0,50.0));
    for _ in 0..60 {h.frame(vec![]);}assert_eq!(h.app.pet_state(),PetState::Curious);assert!(!h.app.open);
}
#[test]
fn mascot_feedback_plays_beside_notice_and_respects_switches(){
    use crate::extras::PetState;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);
    h.app.settings.pet_style=crate::model::PetStyle::Atlas;
    h.app.drink_water();h.app.alert("待办到时间了","测试提醒",4,false);
    for _ in 0..60 {h.frame(vec![]);}assert_eq!(h.app.pet_state(),PetState::Satisfied);
    assert!(h.app.notice.is_some());
    let atlas=h.ctx.tex_manager().read().allocated().find_map(|(id,meta)|(meta.name=="lingqi-expressions").then_some(*id)).expect("表情图集已加载");
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Mesh(mesh) if mesh.texture_id==atlas)),"通知期间灵栖仍应可见");
    for _ in 0..240 {h.frame(vec![]);}assert_eq!(h.app.pet_state(),PetState::Awake);
    h.app.notice=None;for _ in 0..240 {h.frame(vec![]);}assert_eq!(h.app.pet_state(),PetState::Awake);
    h.app.drink_water();h.app.settings.minimal=true;h.frame(vec![]);assert!(h.app.pet.reaction.is_none());
    h.app.settings.minimal=false;h.app.settings.pet=false;h.app.drink_water();h.frame(vec![]);assert!(h.app.pet.reaction.is_none());
}


#[test]
fn mascot_atlas_respects_visibility_and_skin_switches(){
    use crate::pet_expressions::Expression;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.settings.pet=true;h.app.settings.minimal=false;h.app.settings.pet_style=crate::model::PetStyle::Atlas;
    for _ in 0..90 {h.frame(vec![]);}
    for skin in [crate::model::Skin::Classic,crate::model::Skin::Cream,crate::model::Skin::GlassLight] {
        h.app.settings.skin=skin;
        for expression in Expression::ALL {
            h.app.pet.expressions.play(expression);for _ in 0..30 {h.frame(vec![]);}
            assert_eq!(h.app.pet.expressions.current(),Some(expression));
            let atlas=h.ctx.tex_manager().read().allocated().find_map(|(id,meta)|(meta.name=="lingqi-expressions").then_some(*id)).unwrap();
            let drawn=h.shapes.iter().find_map(|shape|match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id==atlas=>Some(mesh),_=>None,
            }).expect("新表情应画在常驻岛上");
            assert!(h.region.contains_rect(drawn.calc_bounds()),"新表情不能撑出岛体");
            // 每个表情单独一张序列图：横向铺满整张，纹理只有一列大小。
            assert!(drawn.vertices[0].uv.x.abs()<0.001);
            assert_eq!(h.ctx.tex_manager().read().meta(atlas).unwrap().size,[144,1440],"只保留正在播放的表情");
        }
    }
    h.app.settings.pet=false;h.frame(vec![]);assert!(h.app.pet.expressions.current().is_none());
    h.app.settings.pet=true;h.app.pet.expressions.play(Expression::Heart);h.app.settings.minimal=true;
    h.frame(vec![]);assert!(h.app.pet.expressions.current().is_none());
}

#[test]
#[ignore="按需导出接入程序后的十二组表情"]
fn mascot_integrated_expression_gallery(){
    use crate::pet_expressions::Expression;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_PET_INTEGRATED_PREVIEW").expect("需指定预览目录"));
    std::fs::create_dir_all(&dest).unwrap();
    for (name,skin) in [("classic",crate::model::Skin::Classic),("cream",crate::model::Skin::Cream),("glass",crate::model::Skin::GlassLight)] {
        let mut h=H::new();h.app.settings.pet=true;h.app.settings.minimal=false;h.app.settings.skin=skin;
        let mut raster=crate::preview_raster::Raster::new(1280,540);
        let mut sequence=0;
        for (index,expression) in Expression::ALL.into_iter().enumerate() {
            for frame in 0..=(expression.duration()*20.0).ceil() as usize {
                if frame==1 {h.app.pet.expressions.play(expression);}
                let t=sequence as f64/20.0;sequence+=1;
                let env=Env{t,clock:"09:41:00".into(),today:"2026-11-06".into(),date:"11月6日 周五".into(),idle_seconds:Some(0.0),..Default::default()};
                let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(WIN_W,WIN_H))),time:Some(t),
                    viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(2.0),..Default::default()})).collect(),..Default::default()};
                let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{
                    let win=ui.max_rect();h.app.frame(ui,win,&env);
                });});
                raster.update_textures(&output.textures_delta);
                if frame>0 {
                    raster.buf.fill([0.95,0.96,0.98,1.0]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
                    let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);let image=image::RgbaImage::from_raw(1280,540,rgba).unwrap();
                    image::imageops::crop_imm(&image,440,0,400,100).to_image().save(dest.join(format!("{name}-{index:02}-{frame:03}.png"))).unwrap();
                }
                raster.free_textures(&output.textures_delta);
            }
        }
    }
}
#[test]
fn todo_due_roundtrip_and_fires_only_once(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.todos.items.push(crate::model::TodoItem{text:"会议 | 准备".into(),done_on:None,due:Some("2026-10-28 09:40".into()),reminded:false,..Default::default()});h.app.todos.save();
    let loaded=crate::model::TodoList::load();assert_eq!(loaded.items[0].text,"会议 | 准备");assert_eq!(loaded.items[0].due.as_deref(),Some("2026-10-28 09:40"));
    h.frame(vec![]);assert!(h.app.todos.items[0].reminded);let n=h.cmds.len();h.frame(vec![]);assert_eq!(n,h.cmds.len());assert!(crate::model::TodoList::load().items[0].reminded);
}
#[test]
fn quiet_water_hours_cover_overnight(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.settings.water_start=22;h.app.settings.water_end=7;
    assert!(h.app.water_hours(23));assert!(h.app.water_hours(2));assert!(!h.app.water_hours(12));
}
#[test]
fn repeated_system_changes_merge_without_losing_history(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);
    h.app.alert("系统音量","音量 20%",5,false);h.frame(vec![]);
    h.app.alert("系统音量","音量 30%",5,false);h.frame(vec![]);
    assert_eq!(h.app.notice.as_ref().unwrap().body,"音量 30%");assert_eq!(h.app.history.len(),2);assert!(h.app.alerts.is_empty());
}

#[test]
fn minimal_takes_precedence_over_mascot(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.settings.pet=true;h.app.settings.minimal=true;
    for _ in 0..120 {h.frame(vec![]);}assert!(h.region.height()<20.0,"启用极简和吉祥物时必须显示细线");
    h.hover(Pos2::new(WIN_W/2.0,2.0),1.0);assert!(h.app.progress()>0.9,"极简线仍要能悬停展开");
}
#[test]
fn water_settings_are_in_the_island_and_editable(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    h.app.go(2,1.0);h.hover(H::local(300.0,60.0),0.8);h.click(H::local(145.0,110.0));assert_eq!(h.app.page,2);assert_eq!(h.app.subpage,Some(2));assert_eq!(h.app.settings_category,1);
    h.hover(H::local(300.0,60.0),0.8);h.click(H::local(357.0,69.0));assert_eq!(h.app.settings.serving_ml,300);
    // 标题旁的切换在设置子页同样可用：切到久坐后第一行就是久坐时长，最后一行时段两边共用。
    h.click(H::local(136.0,19.0));assert!(h.app.sit_mode);for _ in 0..40{h.frame(vec![]);}
    h.hover(H::local(300.0,60.0),0.3);h.click(H::local(357.0,47.0));assert_eq!(h.app.settings.sit_minutes,65);
    h.hover(H::local(300.0,60.0),0.3);h.click(H::local(357.0,113.0));assert!(h.app.health_time,"最后一行进入提醒时段二级页");
    let end=h.app.settings.water_end;h.hover(H::local(300.0,60.0),0.3);h.click(H::local(357.0,47.0));assert_eq!(h.app.settings.water_end,(end+1)%24);
}
#[test]
fn lunch_break_silences_water_and_sit_reminders(){
    use crate::app::SIT_TITLE;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.system_idle=Some(0.0);h.frame(vec![]);
    // 测试时钟固定在 09:41，把午休设成 09:30–10:00。
    h.app.settings.lunch_rest=true;h.app.settings.lunch_start=9*60+30;h.app.settings.lunch_end=10*60;
    h.app.water.remind=true;h.app.last_reminder=h.app.t-2.0*3600.0;h.app.sit_start=h.app.t-2.0*3600.0;
    h.frame(vec![]);assert!(h.app.notice.is_none(),"午休中不应弹喝水或久坐提醒");
    h.app.settings.lunch_rest=false;h.frame(vec![]);
    assert!(h.app.notice.is_some(),"关闭午休后应恢复提醒");
    h.app.notice=None;h.app.alerts.clear();h.app.water.remind=false;h.app.confirm_health_reminder(crate::extras::HealthReminder::Sit);h.app.sit_start=h.t-2.0*3600.0;
    h.app.settings.lunch_rest=true;h.app.settings.lunch_start=10*60;h.app.settings.lunch_end=13*60;
    h.frame(vec![]);assert!(h.app.notice.as_ref().is_some_and(|a|a.title==SIT_TITLE),"午休以外照常提醒");
}
#[test]
fn health_time_subpage_edits_lunch_and_backs_out_one_level(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    h.app.open_settings_category(1);for _ in 0..30{h.frame(vec![]);}
    h.hover(H::local(300.0,60.0),0.3);h.click(H::local(200.0,113.0));assert!(h.app.health_time,"点提醒时段行进入二级页");
    let start=h.app.settings.lunch_start;h.hover(H::local(300.0,60.0),0.3);h.click(H::local(225.0,91.0));assert_eq!(h.app.settings.lunch_start,start+15);
    h.hover(H::local(300.0,60.0),0.3);h.click(H::local(353.0,69.0));assert!(!h.app.settings.lunch_rest,"午休开关可关闭");
    h.hover(H::local(300.0,60.0),0.3);h.click(H::local(20.0,18.0));assert!(!h.app.health_time);assert_eq!(h.app.subpage,Some(2),"返回先回到健康提醒设置");
}
#[test]
fn sit_reminder_waits_for_confirmation_before_restarting(){
    use crate::app::SIT_TITLE;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.system_idle=Some(0.0);h.frame(vec![]);h.app.water.remind=false;
    h.t+=59.0*60.0;h.frame(vec![]);assert!(h.app.notice.is_none(),"未满 60 分钟不提醒: {:?}",h.app.notice.as_ref().map(|a|(a.title.clone(),a.body.clone())));
    h.t+=61.0;h.frame(vec![]);
    assert!(h.app.notice.as_ref().is_some_and(|a|a.title==SIT_TITLE));
    assert!(h.cmds.iter().any(|c|matches!(c,crate::app::Cmd::Notify(t,..)if t==SIT_TITLE)));
    let count=h.app.history.len();
    h.screen_locked=true;h.system_idle=Some(40.0*60.0);h.t+=500.0*60.0;h.frame(vec![]);
    assert!(h.app.notice.is_none());assert_eq!(h.app.history.len(),count,"锁屏和长时间未处理都只保留一次提醒");
    h.app.confirm_health_reminder(crate::extras::HealthReminder::Sit);assert_eq!(h.app.sat_minutes(),0);
    h.screen_locked=false;h.system_idle=Some(0.0);h.t+=59.0*60.0;h.frame(vec![]);assert!(h.app.notice.is_none());
    h.t+=61.0;h.frame(vec![]);assert!(h.app.notice.as_ref().is_some_and(|a|a.title==SIT_TITLE));
}
#[test]
fn sit_reminder_waits_for_focus_and_stops_with_module_or_switch(){
    use crate::app::{Phase,SIT_TITLE,WATER_PAGE};
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.system_idle=Some(0.0);h.frame(vec![]);h.app.water.remind=false;
    h.app.pomo.phase=Phase::Focus;h.app.pomo.remaining=1e7;h.app.pomo.running=true;
    h.t+=61.0*60.0;h.frame(vec![]);assert!(h.app.notice.is_none(),"专注中不打扰: {:?}",h.app.notice.as_ref().map(|a|(a.title.clone(),a.body.clone())));
    h.app.pomo.running=false;h.frame(vec![]);assert!(h.app.notice.as_ref().is_some_and(|a|a.title==SIT_TITLE),"专注结束后补上提醒");
    h.app.notice=None;h.app.settings.sit_remind=false;h.t+=2.0*3600.0;h.frame(vec![]);assert!(h.app.notice.is_none());
    h.app.settings.sit_remind=true;h.app.set_page_visible(WATER_PAGE,false);h.t+=2.0*3600.0;h.frame(vec![]);assert!(h.app.notice.is_none(),"模块关闭即不再提醒");
}
#[test]
fn sit_notice_and_page_buttons_reset_the_timer(){
    use crate::app::SIT_TITLE;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.system_idle=Some(0.0);h.frame(vec![]);
    h.app.sit_start=h.t-70.0*60.0;h.app.alert(SIT_TITLE,"已连续坐了 70 分钟",2,false);for _ in 0..60 {h.frame(vec![]);}
    h.click_text("知道了");assert!(h.app.notice.is_none());assert!((h.app.sit_due()-h.t-3600.0).abs()<5.0);assert_eq!(h.app.water.drunk_ml,0);
    h.app.alert(SIT_TITLE,"已连续坐了 70 分钟",2,false);for _ in 0..60 {h.frame(vec![]);}
    h.click_text("起身了");assert!(h.app.notice.is_none());assert_eq!(h.app.sat_minutes(),0);assert_eq!(h.app.water.drunk_ml,0);
    // 健康页：点「久坐」分段切换，同一位置的主按钮变成「起身了」、开关管久坐；点「喝水」切回
    h.app.sit_start=h.t-30.0*60.0;
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);h.app.go(2,1.0);h.hover(H::local(300.0,60.0),0.8);
    h.click(H::local(124.0,22.0));assert!(h.app.sit_mode);h.hover(H::local(300.0,90.0),0.8);
    h.click(H::local(70.0,110.0));assert_eq!(h.app.sat_minutes(),0);assert_eq!(h.app.water.drunk_ml,0);
    let water_remind=h.app.water.remind;
    h.click(H::local(179.0,46.0));assert!(!h.app.settings.sit_remind);assert_eq!(h.app.water.remind,water_remind,"久坐模式的开关只管久坐");
    assert!(!crate::model::Settings::load().sit_remind,"重新加载后应保留久坐开关");
    h.click(H::local(97.0,22.0));assert!(!h.app.sit_mode);h.hover(H::local(300.0,90.0),0.8);
    h.click(H::local(70.0,110.0));assert_eq!(h.app.water.drunk_ml,250,"回到喝水后按钮可用");
}
#[test]
fn water_notice_buttons_work_without_opening_panel(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);
    h.app.alert("喝水时间到了","该喝水了",2,false);for _ in 0..60 {h.frame(vec![]);}
    h.click_text("喝一口");assert_eq!(h.app.water.drunk_ml,250);assert!(h.app.notice.is_none());
}
#[test]
fn focus_completion_offers_starting_the_break(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);h.app.pomo.running=true;h.app.pomo.remaining=1.0;h.t+=2.0;h.frame(vec![]);
    assert!(h.app.pomo.phase==crate::app::Phase::Rest);assert!(!h.app.pomo.running);assert!(h.app.notice.is_some());
    for _ in 0..60 {h.frame(vec![]);}h.click_text("开始");assert!(h.app.pomo.running);
}
#[test]
fn todo_completed_items_move_down_and_undo_restores_the_original_order(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.todos.items=["已完成甲","待做甲","周一重复","已完成乙","每日待办"].into_iter().map(crate::model::TodoItem::new).collect();
    h.app.todos.items[0].done_on=Some("2026-10-28".into());h.app.todos.items[3].done_on=Some("2026-10-28".into());h.app.todos.items[2].repeat=1;h.app.todos.items[4].repeat=crate::model::REPEAT_ALL;
    let original=h.app.todos.items.iter().map(|t|t.text.clone()).collect::<Vec<_>>();assert_eq!(h.app.todos.display_order("2026-10-28"),vec![1,4,2,0,3],"今天未完成优先，其次其他日期任务，已完成沉底");
    h.app.todos.items[0].toggle("2026-10-28");assert_eq!(h.app.todos.display_order("2026-10-28"),vec![0,1,4,2,3],"取消完成应恢复原有相对顺序");
    h.app.todos.items[4].toggle("2026-10-28");assert_eq!(h.app.todos.display_order("2026-10-28"),vec![0,1,2,3,4]);assert_eq!(h.app.todos.display_order("2026-10-29"),vec![0,1,4,2,3],"重复任务到下一天应回到未完成区");
    assert_eq!(h.app.todos.items.iter().map(|t|t.text.clone()).collect::<Vec<_>>(),original,"只改变显示顺序，不能移动原数据");
}

#[test]
fn todo_edit_choice_can_edit_other_rows_and_completed_items(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.todos.items=["第一项","第二项","已完成任务"].into_iter().map(crate::model::TodoItem::new).collect();h.app.todos.items[2].done_on=Some("2026-10-28".into());
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click_text("改一下");
    assert!(h.app.editing.is_none(),"多条待办应先选择，不能默认修改第一项");h.frame(vec![]);h.click_text("第二项");
    assert_eq!(h.app.editing,Some(1));h.app.buf="第二项已修改".into();h.frame(vec![egui::Event::Key{key:egui::Key::Enter,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
    assert_eq!(h.app.todos.items[0].text,"第一项");assert_eq!(h.app.todos.items[1].text,"第二项已修改");assert!(h.app.todos.items[2].done_on.is_some());
    h.frame(vec![]);h.click_text("改一下");h.frame(vec![]);h.click_text("已完成任务");assert_eq!(h.app.editing,Some(2));
    h.frame(vec![egui::Event::Key{key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);assert!(!h.app.is_editing());assert!(h.app.todos.items[2].done_on.is_some());
}

#[test]
fn todo_edit_choice_keeps_a_single_completed_task_editable(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.todos.items=vec![crate::model::TodoItem::new("已完成任务")];h.app.todos.items[0].done_on=Some("2026-10-28".into());
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click_text("改一下");assert_eq!(h.app.editing,Some(0),"只有一条时可直接编辑，完成状态不能禁用编辑");
}

#[test]
fn todo_edit_choice_reaches_scrolled_items_and_cancels_without_changes(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.todos.items=(0..7).map(|i|crate::model::TodoItem::new(format!("任务 {i}"))).collect();
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click_text("改一下");assert!(h.app.editing.is_none());
    h.hover(H::local(300.0,60.0),0.2);for _ in 0..7{h.wheel(-24.0);}assert_eq!(h.app.page(),4,"选择事项时滚到末尾也不能翻页");h.frame(vec![]);h.click_text("任务 6");assert_eq!(h.app.editing,Some(6));
    h.frame(vec![egui::Event::Key{key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);assert!(!h.app.is_editing());assert_eq!(h.app.todos.items[6].text,"任务 6");assert!(h.app.todos.items.iter().all(|t|t.done_on.is_none()));
}

#[test]
fn todo_completed_reordering_does_not_complete_the_next_item_on_a_double_click(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.todos.items=["任务甲","任务乙","任务丙"].into_iter().map(crate::model::TodoItem::new).collect();h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);
    let point=H::local(212.0,46.0);h.click(point);h.frame(vec![]);h.click(point);h.frame(vec![]);
    assert!(h.app.todos.items[0].done_on.is_some());assert!(h.app.todos.items[1].done_on.is_none(),"首项下移后，快速连点不能误完成下一项");
    h.click_text("任务甲");h.frame(vec![]);assert!(h.app.todos.items.iter().all(|t|t.done_on.is_none()));assert_eq!(h.app.todos.display_order("2026-10-28"),vec![0,1,2]);
}

#[test]
fn todo_edit_choice_can_cancel_without_toggling_or_deleting_items(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.todos.items=["任务甲","任务乙"].into_iter().map(crate::model::TodoItem::new).collect();h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);
    h.click_text("改一下");h.frame(vec![]);h.click_text("取消");assert!(!h.app.is_editing());assert_eq!(h.app.todos.items.len(),2);assert!(h.app.todos.items.iter().all(|t|t.done_on.is_none()));
    h.frame(vec![]);h.click_text("改一下");h.frame(vec![egui::Event::Key{key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);assert!(!h.app.is_editing());assert_eq!(h.app.todos.items.len(),2);
}

#[test]
fn todo_edit_choice_keeps_the_selected_item_when_an_earlier_item_is_deleted(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.todos.items=["第一项","第二项","第三项"].into_iter().map(crate::model::TodoItem::new).collect();h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);
    h.click_text("改一下");h.frame(vec![]);h.click_text("第二项");h.frame(vec![]);h.click(h.ctx.read_response(egui::Id::new(("card",true,("todo-delete",0usize)))).unwrap().rect.center());
    assert_eq!(h.app.editing,Some(0),"删除前面的条目后，编辑目标仍须是第二项");h.app.buf="第二项已修改".into();h.frame(vec![egui::Event::Key{key:egui::Key::Enter,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);assert_eq!(h.app.todos.items.iter().map(|t|t.text.as_str()).collect::<Vec<_>>(),["第二项已修改","第三项"]);
}

#[test]
#[ignore="按需导出待办完成排序和点选编辑的实际画面"]
fn todo_edit_choice_gallery(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_TODO_PREVIEW_DIR").expect("需指定预览目录"));std::fs::create_dir_all(&dest).unwrap();
    for skin in [crate::model::Skin::Classic,crate::model::Skin::Cream]{for choosing in [false,true]{
        let mut h=H::new();h.app.settings.skin=skin;h.app.todos.items=["整理会议资料","确认本周安排","已经完成的任务"].into_iter().map(crate::model::TodoItem::new).collect();h.app.todos.items[2].done_on=Some("2026-11-06".into());h.app.show_page(4);h.app.todo_choosing=choosing;
        let mut raster=feature_raster();shoot_feature(&mut h,&mut raster,&dest,&format!("todo-{}-{}",skin.key(),if choosing{"choose"}else{"list"}),90,Some(H::local(175.0,70.0)),"2026-11-06");
    }}
}

#[test]
fn task_editor_saves_and_cancels_inside_island(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);
    h.app.todos.items.push(crate::model::TodoItem{text:"准备会议".into(),done_on:None,due:None,reminded:false,..Default::default()});h.app.go(4,1.0);h.hover(H::local(300.0,60.0),0.8);h.app.open_task_dialog(0);
    h.hover(H::local(300.0,60.0),0.2);assert!(h.app.is_editing());h.click(H::local(326.0,110.0));assert!(!h.app.is_editing());assert_eq!(h.app.todos.items[0].text,"准备会议");
    h.app.open_task_dialog(0);h.frame(vec![egui::Event::Key{key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);assert!(!h.app.is_editing());
}
#[test]
fn focus_timer_gets_room_in_the_collapsed_island(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();for _ in 0..90 {h.frame(vec![]);}
    let before=h.app.current_width();h.app.pomo.running=true;
    for _ in 0..100 {h.frame(vec![]);}assert!(h.app.current_width()>before+20.0);assert!(h.app.pomo.remaining<1500.0);
}

#[test]
fn switching_to_rest_waits_for_the_start_button(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);h.app.go(1,1.0);h.hover(H::local(260.0,70.0),1.0);
    h.click(H::local(296.0,106.5));
    assert!(h.app.pomo.phase==crate::app::Phase::Rest);
    assert!(!h.app.pomo.running,"切换阶段应等待用户点击开始");
}



#[test]
fn focus_page_selects_stopped_phases_and_starts_only_from_play(){
    use crate::app::Phase;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.settings.focus_minutes=30;h.app.settings.rest_minutes=7;
    h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);h.app.go(1,1.0);h.hover(H::local(260.0,70.0),1.0);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="休息")));
    h.click(H::local(336.0,106.5));
    assert!(h.app.pomo.phase==Phase::Rest && !h.app.pomo.running);
    assert_eq!(h.app.pomo.remaining,420.0,"休息采用设置中的时长");
    h.click(H::local(264.0,106.5));assert!(h.app.pomo.running);
    h.click(H::local(264.0,106.5));assert!(!h.app.pomo.running);
    let paused=h.app.pomo.remaining;for _ in 0..30 {h.frame(vec![]);}assert_eq!(h.app.pomo.remaining,paused);
    h.click(H::local(264.0,106.5));assert!(h.app.pomo.running && h.app.pomo.phase==Phase::Rest);
    h.click(H::local(336.0,106.5));assert!(h.app.pomo.running,"点击当前阶段不重置计时");
    h.click(H::local(192.0,106.5));assert!(h.app.pomo.phase==Phase::Focus && !h.app.pomo.running);
    assert_eq!(h.app.pomo.remaining,1800.0);
    h.click(H::local(264.0,106.5));assert!(h.app.pomo.running);
    h.click(H::local(336.0,106.5));assert!(h.app.pomo.phase==Phase::Rest && !h.app.pomo.running);
    h.click(H::local(264.0,106.5));assert!(h.app.pomo.running);
    h.click(H::local(329.0,50.0));assert!(!h.app.pomo.running && h.app.pomo.phase==Phase::Rest);
    assert_eq!(h.app.pomo.remaining,420.0,"重置应保留休息阶段");
}

#[test]
fn collapsed_focus_uses_tomato_and_rest_uses_a_white_cup(){
    use crate::app::Phase;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    for phase in [Phase::Focus,Phase::Rest] {
        h.app.pomo.phase=phase;h.app.pomo.remaining=h.app.phase_secs(phase);h.app.pomo.running=true;
        for _ in 0..120 {h.frame(vec![]);}
        let secs=h.app.pomo.remaining.ceil() as u32;let text=format!("{:02}:{:02}",secs/60,secs%60);
        let color=if phase==Phase::Focus{crate::theme::RED_TOP}else{crate::theme::SOFT_GREEN};
        assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==text && t.fallback_color==color)),"常驻倒计时颜色应跟随阶段");
        let tomato=h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Mesh(m) if m.vertices.iter().any(|v|v.color==crate::theme::RED_TOP)));
        if phase==Phase::Focus {assert!(tomato,"专注时应显示红色番茄水位");}
        else {
            assert!(!tomato,"休息时应替换番茄");
            assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.fill==egui::Color32::from_rgb(245,242,234) && (7.0..13.0).contains(&r.rect.width()) && (5.0..10.0).contains(&r.rect.height()))),"休息时应显示白瓷茶杯");
        }
        assert!((h.hit.height()-34.0).abs()<0.1,"图标不能撑高常驻岛");
    }
}

#[test]
fn focus_and_rest_completion_prepare_the_configured_next_phase_without_starting(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);
    h.app.settings.rest_minutes=7;h.app.settings.focus_minutes=30;
    for _ in 0..5 {
        h.app.pomo.phase=crate::app::Phase::Focus;h.app.pomo.running=true;h.app.pomo.remaining=0.01;h.frame(vec![]);
        assert!(h.app.pomo.phase==crate::app::Phase::Rest && !h.app.pomo.running);
        assert_eq!(h.app.pomo.remaining,420.0);assert!(h.app.notice.is_some());
        h.app.pomo.running=true;h.app.pomo.remaining=0.01;h.frame(vec![]);
        assert!(h.app.pomo.phase==crate::app::Phase::Focus && !h.app.pomo.running);
        assert_eq!(h.app.pomo.remaining,1800.0);
    }
}

#[test]
fn rest_settings_keep_the_existing_duration_and_save_the_new_key(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let _h=H::new();
    assert_eq!(crate::model::Settings::default().rest_minutes,5);
    let path=crate::model::config_dir().join("settings.txt");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path,"focus_minutes=30\nshort_minutes=7\nlong_minutes=17\n").unwrap();
    let settings=crate::model::Settings::load();assert_eq!(settings.rest_minutes,7);
    settings.save();let saved=std::fs::read_to_string(&path).unwrap();
    assert!(saved.lines().any(|line|line=="rest_minutes=7"));
    assert!(!saved.contains("short_minutes") && !saved.contains("long_minutes"));
    let reloaded=crate::model::Settings::load();assert_eq!(reloaded.rest_minutes,7);assert_eq!(reloaded.focus_minutes,30);
}





#[test]
fn volume_hud_drag_maps_pointer_to_percentage(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.frame(vec![]);
    h.app.alert("系统音量","音量 0%",5,false);
    for _ in 0..60 {h.frame(vec![]);}
    assert!(!h.app.open);
    h.click_notice_badge();
    // 缩短后的系统音量条仍支持点击和拖动。
    let (x0,y)=if h.app.compact_notices(){(h.notice_button().left()+crate::app::HUD_TRACK_X,55.0)}else{(WIN_W/2.0-h.app.current_width()/2.0+h.app.pet_space()+crate::app::HUD_TRACK_X,19.0)};
    let press=Pos2::new(x0+52.0,y);
    h.cursor=Some(press);h.frame(vec![egui::Event::PointerMoved(press)]);
    h.frame(vec![egui::Event::PointerButton{pos:press,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
    let to=Pos2::new(x0+105.0,y);
    h.cursor=Some(to);h.frame(vec![egui::Event::PointerMoved(to)]);
    h.frame(vec![egui::Event::PointerButton{pos:to,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
    let last=h.cmds.iter().rev().find_map(|c|match c{crate::app::Cmd::SetVolume(v)=>Some(*v),_=>None});
    assert_eq!(last,Some(70));
}

#[test]
fn display_choice_saves_and_keeps_disconnected_selection(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.app.settings_open=true;h.app.settings_tab=crate::extras::SETTINGS_TAB_POSITION;h.ctx.set_embed_viewports(false);
    let bounds=crate::display_layout::Bounds{left:0,top:0,right:1920,bottom:1080};
    h.app.displays=vec![
        crate::display_layout::Display{id:"internal".into(),label:"显示器 1 · 主屏".into(),bounds,work:bounds,auto_bottom:0,primary:true,dpi:96},
        crate::display_layout::Display{id:r"\\?\DISPLAY#external".into(),label:"显示器 2 · 外接屏".into(),bounds,work:bounds,auto_bottom:0,primary:false,dpi:96},
    ];
    let mut raster=crate::preview_raster::Raster::new(690,560);
    fn frame(h:&mut H,raster:&mut crate::preview_raster::Raster,events:Vec<egui::Event>){
        h.t+=1.0/60.0;let time=h.t;
        let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            *capture.lock().unwrap()=Some(ctx.run(egui::RawInput{time:Some(time),viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,560.0))),events:events.clone(),viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()},|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        let root=h.ctx.run(egui::RawInput{time:Some(time),..Default::default()},|ctx|h.app.settings_ui(ctx,false));let child=output.lock().unwrap().take().unwrap();
        raster.update_textures(&root.textures_delta);raster.update_textures(&child.textures_delta);h.shapes=child.shapes;
    }
    // 设置页内容超过一屏时先向下滚动，直到目标完整出现在窗口内。
    fn reveal(h:&mut H,raster:&mut crate::preview_raster::Raster,label:&str)->Option<Rect>{
        let find=|h:&H|h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()==label=>Some(t.visual_bounding_rect()),_=>None}).filter(|r|r.bottom()<550.0);
        for _ in 0..20 {
            if let Some(r)=find(h){return Some(r);}
            frame(h,raster,vec![egui::Event::PointerMoved(Pos2::new(450.0,300.0)),egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:Vec2::new(0.0,-60.0),modifiers:Default::default()}]);
            for _ in 0..6{frame(h,raster,vec![]);}
        }
        find(h)
    }
    fn click(h:&mut H,raster:&mut crate::preview_raster::Raster,label:&str){
        let pos=reveal(h,raster,label).map(|r|r.center()).unwrap_or_else(||panic!("未找到：{label}"));
        for pressed in [true,false]{frame(h,raster,vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}]);}
        frame(h,raster,vec![]);
    }
    for _ in 0..3{frame(&mut h,&mut raster,vec![]);}
    assert!(h.app.settings.display_id.is_empty());click(&mut h,&mut raster,"跟随主屏");
    for _ in 0..20{frame(&mut h,&mut raster,vec![]);}
    if let Some(path)=std::env::var_os("LINGQI_DISPLAY_PREVIEW"){
        raster.buf.fill([0.0,0.0,0.0,1.0]);raster.draw(&h.ctx.tessellate(h.shapes.clone(),1.0),1.0);
        let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);image::save_buffer(path,&rgba,690,560,image::ColorType::Rgba8).unwrap();
    }
    click(&mut h,&mut raster,"显示器 2 · 外接屏");assert_eq!(h.app.settings.display_id,r"\\?\DISPLAY#external");
    assert_eq!(crate::model::Settings::load().display_id,h.app.settings.display_id);
    h.app.displays.pop();frame(&mut h,&mut raster,vec![]);
    reveal(&mut h,&mut raster,"所选屏幕未连接");
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()=="所选屏幕未连接")));
    assert_eq!(crate::display_layout::selected(&h.app.displays,&h.app.settings.display_id).unwrap().id,"internal");
    click(&mut h,&mut raster,"所选屏幕未连接");click(&mut h,&mut raster,"跟随主屏");
    assert!(crate::model::Settings::load().display_id.is_empty());
}













#[test]
fn hud_expands_smoothly_from_collapsed_width(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    for _ in 0..60 { h.frame(vec![]); }
    let before=h.app.current_width();
    h.app.alert("大写锁定","已开启",5,false);
    h.frame(vec![]);
    assert!(h.app.current_width()-before < 30.0, "提示条不能在第一帧突然跳到目标宽度");
}


#[test]
fn hud_repeated_volume_changes_do_not_restart_entrance(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.frame(vec![]);
    h.app.alert("系统音量","音量 20%",5,false);
    for _ in 0..45 {h.frame(vec![]);}
    let before=h.app.current_width();
    h.app.alert("系统音量","音量 30%",5,false);
    h.frame(vec![]);
    assert!((h.app.current_width()-before).abs()<5.0);
    assert_eq!(h.app.notice.as_ref().unwrap().body,"音量 30%");
}

#[test]
fn hud_shrinks_smoothly_after_expiration(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    for _ in 0..90 {h.frame(vec![]);}let collapsed=h.app.current_width();
    h.app.alert("大写锁定","已开启",5,false);
    for _ in 0..60 {h.frame(vec![]);}
    let before=h.app.current_width();
    h.t+=2.0;
    h.frame(vec![]);
    assert!(h.app.notice.is_none());
    assert!(h.app.current_width()>collapsed+5.0);
    assert!(h.app.current_width()<before);
}

#[test]
fn focus_settings_are_inside_focus_page(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();h.hover(Pos2::new(WIN_W/2.0,12.0),1.0);h.app.go(1,1.0);h.hover(H::local(300.0,60.0),0.8);h.click(H::local(110.0,18.0));assert_eq!(h.app.page,1);assert_eq!(h.app.subpage,Some(1));
}














#[test]
fn language_notice_text_is_vertically_centered() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();h.frame(vec![]);
    h.app.alert("键盘输入语言","英语",5,false);
    for _ in 0..60 {h.frame(vec![]);}
    h.click_notice_badge();
    fn find(shapes:&[egui::epaint::ClippedShape])->Option<Rect>{
        shapes.iter().find_map(|s|match &s.shape {
            egui::Shape::Text(text) if text.galley.text()=="英语"=>Some(text.visual_bounding_rect()),
            _=>None,
        })
    }
    let text=find(&h.shapes).expect("输入语言状态应可见");
    assert!((text.center().y-19.0).abs()<1.0,"文字应与图标垂直居中：{text:?}");
}

#[test]
fn todo_notice_view_locates_the_reminded_task() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.todos.items=(0..7).map(|i|crate::model::TodoItem{text:format!("任务 {i}"),done_on:None,due:if i==6{Some("2026-10-28 09:40".into())}else{None},reminded:false,..Default::default()}).collect();
    for _ in 0..60 {h.frame(vec![]);}
    h.click_text("查看");
    assert_eq!(h.app.page,4);
    assert!(h.app.todo_first>=4,"打开待办页时应能看到收到提醒的任务");
}


#[cfg(feature = "material")]
const SHORTCUT_FIXTURE_ID: &str = "example.page-shortcuts";

#[cfg(feature = "material")]
fn install_page_shortcuts_fixture(h: &mut H) {
    let source = crate::model::config_dir().join("shortcut-plugin-source");
    std::fs::create_dir_all(&source).unwrap();
    let manifest = serde_json::json!({
        "id": SHORTCUT_FIXTURE_ID, "name":"快捷切换", "author":"测试", "version":"1.0.0",
        "api":1, "runtime":"assets", "entry":"shortcuts.json", "cover":"cover.png",
        "owner":"pages", "capabilities":["navigation.shortcuts"], "dataVersion":1,
        "permissions":{}, "settings":[]
    });
    let colors = crate::app::PAGE_SHORTCUT_KEYS.iter().enumerate()
        .map(|(index, &key)| (key, format!("#D0{:02X}7D", index*12))).collect::<std::collections::BTreeMap<_,_>>();
    let path = source.join("plugin.json");
    std::fs::write(&path,manifest.to_string()).unwrap();
    std::fs::write(source.join("shortcuts.json"),serde_json::json!({"colors":colors}).to_string()).unwrap();
    std::fs::write(source.join("cover.png"),include_bytes!("../spike/examples/payday/cover.png")).unwrap();
    h.app.plugins.install(&path).unwrap();
}


#[cfg(feature = "material")]
#[test]
fn bottom_page_shortcuts_share_a_row_with_dots_on_every_dock() {
    use crate::app::{FOCUS_PAGE,MUSIC_PAGE,TODO_PAGE};
    use crate::model::Dock;

    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let pages=[TODO_PAGE,MUSIC_PAGE,FOCUS_PAGE];
    let control=|h:&H,kind:&str,page:usize| {
        h.ctx.read_response(egui::Id::new(("card",true,(kind,page)))).map(|r|r.interact_rect)
    };
    let highlight=|h:&H, area:Rect| h.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Rect(rect) if rect.fill.a()>0 && area.contains_rect(rect.rect)
            && rect.rect.center().distance(area.center())<0.1 => Some(rect.fill),
        _ => None,
    }).expect("当前功能应有选中底色");
    let settle=|h:&mut H| {
        for _ in 0..100 {frame_control_draw(h,None,vec![]);}
    };
    let click=|h:&mut H,pos:Pos2| {
        frame_control_draw(h,Some(pos),vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true,false] {
            frame_control_draw(h,Some(pos),vec![egui::Event::PointerButton {
                pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default(),
            }]);
        }
    };

    for dock in [Dock::Top,Dock::Bottom,Dock::Left,Dock::Right] {
        let mut h=H::new();
        install_page_shortcuts_fixture(&mut h);
        h.app.plugins.enable(SHORTCUT_FIXTURE_ID,true).unwrap();
        h.app.settings.dock=dock;
        assert!(h.app.settings.show_page_shortcuts);
        h.app.settings.visible_pages=std::array::from_fn(|page|pages.contains(&page));
        h.app.settings.page_order.rotate_left(TODO_PAGE);
        h.app.show_page(TODO_PAGE);
        settle(&mut h);
        assert_eq!(h.app.expanded_height(),PH+24.0);

        let shortcuts=pages.map(|page|control(&h,"page-shortcut",page).expect("应显示已启用页面的快捷图标"));
        let dots=pages.map(|page|control(&h,"dot",page).expect("应显示已启用页面的页码点"));
        assert!(shortcuts.windows(2).all(|pair|pair[0].right()<=pair[1].left()));
        assert!(dots.windows(2).all(|pair|pair[0].center().x<pair[1].center().x));
        assert!(shortcuts[2].right()<=dots[0].left());
        let row_y=shortcuts[0].center().y;
        assert!(shortcuts.iter().chain(&dots).all(|rect|(rect.center().y-row_y).abs()<0.1),"{dock:?} 停靠的快捷图标和页码点应在同一排");
        for page in 0..h.app.settings.visible_pages.len() {
            if !pages.contains(&page) {
                assert!(control(&h,"page-shortcut",page).is_none());
                assert!(control(&h,"dot",page).is_none());
            }
        }

        let todo_color=highlight(&h,shortcuts[0]);
        click(&mut h,shortcuts[1].center());
        assert_eq!(h.app.page(),MUSIC_PAGE);
        settle(&mut h);
        let music=control(&h,"page-shortcut",MUSIC_PAGE).unwrap();
        assert_ne!(highlight(&h,music),todo_color,"音乐和待办的选中底色应不同");
        let dot=control(&h,"dot",FOCUS_PAGE).unwrap();
        click(&mut h,dot.center());
        assert_eq!(h.app.page(),FOCUS_PAGE);

        h.app.settings.show_page_shortcuts=false;
        settle(&mut h);
        assert_eq!(h.app.expanded_height(),PH);
        assert!(control(&h,"page-shortcut",FOCUS_PAGE).is_none());
        assert!(control(&h,"dot",FOCUS_PAGE).is_some());

        h.app.settings.show_page_shortcuts=true;
        h.app.settings.visible_pages=std::array::from_fn(|page|page==FOCUS_PAGE);
        settle(&mut h);
        assert_eq!(h.app.expanded_height(),PH);
        assert!(control(&h,"page-shortcut",FOCUS_PAGE).is_none());
        assert!(control(&h,"dot",FOCUS_PAGE).is_none());
    }
}

#[cfg(feature = "material")]
#[test]
fn bottom_page_shortcuts_fit_the_performance_panel_with_all_pages_enabled() {
    use crate::app::{PAGE_NAMES, PERFORMANCE_PAGE};
    use crate::model::Dock;

    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for dock in [Dock::Top,Dock::Bottom] {
        let mut h=H::new();
        install_page_shortcuts_fixture(&mut h);
        h.app.plugins.enable(SHORTCUT_FIXTURE_ID,true).unwrap();
        h.app.settings.dock=dock;
        h.app.settings.visible_pages.fill(true);
        h.app.show_page(PERFORMANCE_PAGE);
        for _ in 0..100 {frame_control_draw(&mut h,None,vec![]);}
        let response=|kind:&str,page| h.ctx.read_response(egui::Id::new(("card",true,(kind,page))))
            .expect("展开完整面板应显示快捷图标和页码点");
        let center_y=response("page-shortcut",PERFORMANCE_PAGE).interact_rect.center().y;
        for page in 0..PAGE_NAMES.len() {
            let icon=response("page-shortcut",page).interact_rect;
            let dot=response("dot",page).interact_rect;
            assert!(icon.width()>=25.0);
            assert!((icon.center().y-center_y).abs()<0.1 && (dot.center().y-center_y).abs()<0.1);
            assert!(icon.top()>=0.0 && dot.bottom()<=WIN_H);
        }
    }
}

#[test]
fn page_arrows_stay_at_same_position_across_pages_and_docks() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    use crate::model::Dock;

    for dock in [Dock::Top,Dock::Bottom,Dock::Left,Dock::Right] {
        let mut h=H::new();
        h.app.settings.dock=dock;
        h.app.settings.visible_pages.fill(true);
        // 已下线的页不再参与导航：音乐/邮箱/额度/日程/蓝牙。
        for page in crate::model::REMOVED_PAGES {h.app.settings.visible_pages[page]=false;}
        let pages:Vec<usize>=h.app.settings.page_order.iter().copied()
            .filter(|page|h.app.settings.visible_pages[*page]).collect();
        h.app.show_page(pages[0]);
        for _ in 0..100 {frame_control_draw(&mut h,None,vec![]);}

        let arrow_rects=|h:&H| ["nav-l","nav-r"].map(|id| {
            let rect=h.ctx.read_response(egui::Id::new(("card",true,id)))
                .expect("展开后应显示翻页箭头").interact_rect;
            assert!((rect.size()-Vec2::splat(26.0)).length()<0.01);
            rect
        });
        let fixed=arrow_rects(&h);
        for (arrow,targets) in [(1,pages[1..].to_vec()),(0,pages[..pages.len()-1].iter().rev().copied().collect())] {
            let pos=fixed[arrow].center();
            frame_control_draw(&mut h,Some(pos),vec![egui::Event::PointerMoved(pos)]);
            for target in targets {
                for pressed in [true,false] {
                    frame_control_draw(&mut h,Some(pos),vec![egui::Event::PointerButton {
                        pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default(),
                    }]);
                }
                assert_eq!(h.app.page(),target,"{dock:?} 停靠时应能在同一个位置连续点击翻页");
                for _ in 0..20 {
                    frame_control_draw(&mut h,Some(pos),vec![]);
                    for (actual,expected) in arrow_rects(&h).into_iter().zip(fixed) {
                        assert!((actual.center()-expected.center()).length()<0.01,
                            "{dock:?} 停靠切到第 {target} 页时，翻页箭头位置发生移动");
                    }
                }
            }
        }
    }
}

#[test]
fn header_grip_has_uniform_fill_at_fractional_dpi() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let bounds=crate::display_layout::Bounds{left:0,top:0,right:1920,bottom:1080};
    let mut h=H::new();
    let ppp=1.75;
    island_drag_header(&mut h,bounds,ppp);
    let grip=h.shapes.iter().find(|s|matches!(&s.shape,egui::Shape::Rect(r)
        if (r.rect.height()-2.6).abs()<0.01 && (r.rect.top()-7.5).abs()<0.01)).expect("拖动短条应可见");
    let egui::Shape::Rect(r)=&grip.shape else {unreachable!()};
    let mut raster=crate::preview_raster::Raster::new((r.rect.right()*ppp).ceil() as usize+1,32);
    raster.draw(&h.ctx.tessellate(vec![grip.clone()],ppp),ppp);
    let row=(r.rect.center().y*ppp).floor() as usize*raster.w;
    let expected=r.fill.r() as f32/255.0;
    for x in (r.rect.left()*ppp+3.0).ceil() as usize..(r.rect.right()*ppp-3.0).floor() as usize {
        assert!((raster.buf[row+x][0]-expected).abs()<0.01,"横条中线像素 {x} 应与填充色一致");
    }
}




#[test]
fn page_resize_keeps_the_expanded_header_attached_to_the_panel() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for dock in crate::model::Dock::ALL {
        let mut h=H::new();
        h.app.settings.dock=dock;
        h.app.settings.visible_pages.fill(true);
        h.app.show_page(1);
        for _ in 0..110 {frame_control_draw(&mut h,None,vec![]);}
        let size=if dock.is_side(){Vec2::new(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H)}else{Vec2::new(WIN_W,WIN_H)};
        let win=Rect::from_min_size(Pos2::ZERO,size);
        for (page,frames) in [(8,48),(3,5),(7,7),(6,5),(1,45)] {
            h.app.go(page,1.0);
            for frame in 0..frames {
                frame_control_draw(&mut h,None,vec![]);
                assert!(h.app.reveal>0.99,"{dock:?} 切到第 {page} 页第 {frame} 帧误触发展开动画：{}",h.app.reveal);
                let shell=h.app.island_rect(win,h.app.w.pos,h.app.h.pos);
                // egui 在帧末交换命中缓存，逐帧位置应直接从本帧绘制的齿轮路径读取。
                let gear=h.shapes.iter().find_map(|shape|match &shape.shape {
                    egui::Shape::Path(path) if path.closed && path.points.len()==64=>{
                        let bounds=shape.shape.visual_bounding_rect();
                        ((bounds.center().x-shell.left()-394.0).abs()<1.0).then_some(bounds.center())
                    },
                    _=>None,
                }).expect("切页过程中设置图标应持续绘制");
                assert!((gear.y-shell.top()-18.0).abs()<1.0,"{dock:?} 页 {page} 帧 {frame}，齿轮 {gear:?}，面板 {shell:?}");
            }
        }
    }
}

#[test]
fn hold_open_side_docks_only_expose_complete_hit_targets() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for dock in [crate::model::Dock::Left,crate::model::Dock::Right] {
        let mut h=H::new();
        h.app.settings.dock=dock;
        h.app.settings.island_size=105;
        h.app.show_page(3);
        let id=egui::Id::new(("card",true,"hold-open"));
        for _ in 0..100 {
            frame_control_draw(&mut h,None,vec![]);
            if let Some(button)=h.ctx.read_response(id) {
                assert!((button.rect.width()-25.0).abs()<0.01 && (button.rect.height()-26.0).abs()<0.01);
            }
        }
        let pin=h.ctx.read_response(id).unwrap().rect.center();
        frame_control_draw(&mut h,Some(pin),vec![egui::Event::PointerMoved(pin)]);
        for pressed in [true,false] {
            let event=egui::Event::PointerButton{pos:pin,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()};
            frame_control_draw(&mut h,Some(pin),vec![event]);
        }
        h.app.go(4,1.0);
        for _ in 0..100 {frame_control_draw(&mut h,None,vec![]);}
        assert!(h.ctx.read_response(id).is_some());
    }
}


fn frame_control_draw(h:&mut H,cursor:Option<Pos2>,events:Vec<egui::Event>) {
    use crate::side_dock::{SIDE_H,SIDE_W};
    let size=if h.app.settings.dock.is_side(){Vec2::new(SIDE_W,SIDE_H)}else{Vec2::new(WIN_W,WIN_H)};
    let ppp=1.25*h.app.settings.island_scale();
    h.t+=1.0/120.0;
    let env=Env{t:h.t,clock:"09:41:00".into(),today:"2026-10-28".into(),cursor,
        opaque_surface:true,..Default::default()};
    let win=Rect::from_min_size(Pos2::ZERO,size);
    let raw=egui::RawInput{screen_rect:Some(win),time:Some(h.t),events,focused:true,
        viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{
            native_pixels_per_point:Some(ppp),..Default::default()
        })).collect(),..Default::default()};
    let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{
        h.app.frame(ui,win,&env);
    });});
    h.shapes=output.shapes;
}
#[test]
fn frame_control_cream_startup_notice_keeps_view_action_whole() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.skin=crate::model::Skin::Cream;
    h.app.settings.island_shape=crate::model::IslandShape::Notch;
    h.app.settings.island_size=80;
    h.app.settings.render_compat=true;
    h.app.settings.pet=false;
    h.app.todos.items.push(crate::model::TodoItem{text:"窗口边界逐帧验证".into(),due:Some("2026-10-28 09:40".into()),..Default::default()});
    let action=egui::Id::new(("card",true,"notice-action"));
    for _ in 0..120 {
        frame_control_draw(&mut h,None,vec![]);
        for part in &h.shapes {
            if let egui::Shape::Text(t)=&part.shape {
                let bounds=t.visual_bounding_rect();
                if t.galley.text()=="查看" && part.clip_rect.intersect(bounds).is_positive() {
                    assert!(part.clip_rect.contains_rect(bounds),"通知动画不能截掉查看文字");
                }
            }
        }
        if let Some(r)=h.ctx.read_response(action).filter(|r|r.interact_rect.is_positive()) {
            assert!((r.interact_rect.size()-Vec2::new(60.0,20.0)).length()<0.01,"查看按钮不能只有一部分可点击");
        }
    }
    assert!(h.app.todos.items[0].reminded,"该用例必须触发启动时已有的待办");
    let button=h.ctx.read_response(action).expect("待办通知展开后应有查看按钮");
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)
        if t.galley.text()=="查看" && s.clip_rect.contains_rect(t.visual_bounding_rect()))));
    let pos=button.interact_rect.center();
    frame_control_draw(&mut h,Some(pos),vec![egui::Event::PointerMoved(pos)]);
    for pressed in [true,false] {
        frame_control_draw(&mut h,Some(pos),vec![egui::Event::PointerButton{
            pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()
        }]);
    }
    assert_eq!(h.app.page,4,"稳定态的查看按钮仍应打开待办页");
}

#[test]
fn hud_titles_and_states_share_the_icon_center_without_overlapping() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for (title,body,left,right) in [
        ("网络连接","Wi-Fi · 网络已连接","Wi-Fi","网络已连接"),
        ("网络连接","以太网 · 网络已断开","以太网","网络已断开"),
        ("大写锁定","已开启","大写锁定","已开启"),
        ("键盘输入语言","中文","输入语言","中文"),
        ("电源已连接","当前电量 76%","电源已连接","当前电量 76%"),
        ("已切换电池供电","当前电量 76%","电池供电","当前电量 76%"),
    ] {
        let mut h=H::new();h.frame(vec![]);h.app.alert(title,body,5,false);
        for _ in 0..60{h.frame(vec![]);}
        h.click_notice_badge();
        let find=|needle:&str|h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()==needle=>Some(t.visual_bounding_rect()),_=>None}).unwrap_or_else(||panic!("未显示状态文字：{needle}"));
        let l=find(left);let r=find(right);
        assert!((l.center().y-19.0).abs()<1.0 && (r.center().y-19.0).abs()<1.0,"{title} 应垂直居中");
        assert!(l.right()+8.0<=r.left(),"{title} 的标题和状态应留出间距");
    }
}



#[test]
fn adding_a_todo_saves_content_then_offers_an_optional_reminder() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for remind in [false,true] {
        let mut h=H::new();h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);
        h.click(H::local(70.0,109.0));h.frame(vec![egui::Event::Text("提交产品方案".into())]);
        h.frame(vec![egui::Event::Key{key:egui::Key::Enter,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
        assert_eq!(h.app.todos.items[0].text,"提交产品方案");
        assert!(h.app.todos.items[0].due.is_none());
        assert!(h.app.task_dialog.is_some(),"保存内容后应进入可选提醒步骤");
        assert_eq!(crate::model::TodoList::load().items[0].text,"提交产品方案");
        h.frame(vec![egui::Event::Key{key:egui::Key::Enter,physical_key:None,pressed:false,repeat:false,modifiers:Default::default()}]);
        if remind {
            let initial=chrono::NaiveDateTime::parse_from_str(&h.app.task_dialog.as_ref().unwrap().picker.due(),"%Y-%m-%d %H:%M").unwrap();
            h.hover(H::local(235.0,77.0),0.4);h.wheel(-40.0);h.picker_click("完成");
            let expected=(initial+chrono::Duration::days(1)).format("%Y-%m-%d %H:%M").to_string();
            assert_eq!(h.app.todos.items[0].due.as_deref(),Some(expected.as_str()));
        }else{h.picker_click("暂不提醒");assert!(h.app.todos.items[0].due.is_none());}
        assert!(h.app.task_dialog.is_none());
        assert!(h.app.todos.items[0].done_on.is_none());
        assert_eq!(crate::model::TodoList::load().items[0].due,h.app.todos.items[0].due);
    }
}

#[test]
fn todo_clock_button_sets_a_reminder_without_completing_the_task() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.todos.items=vec![crate::model::TodoItem{text:"已有任务".into(),done_on:None,due:None,reminded:false,..Default::default()}];
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);h.click(H::local(343.0,46.0));
    assert!(h.app.task_dialog.is_some(),"每项待办应有可见的提醒入口");
    assert!(h.app.todos.items[0].done_on.is_none(),"时钟入口不能同时勾选任务");
    h.frame(vec![egui::Event::Key{key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
    assert!(h.app.task_dialog.is_none());assert_eq!(h.app.todos.items[0].text,"已有任务");
}



#[test]
fn todo_reminder_picker_is_embedded_in_the_island(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.settings.show_page_shortcuts=false;
    h.app.todos.items=vec![crate::model::TodoItem{text:"准备会议".into(),done_on:None,due:None,reminded:false,..Default::default()}];
    h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);
    h.ctx.set_embed_viewports(false);
    egui::Context::set_immediate_viewport_renderer(|_,_|panic!("待办提醒必须在岛内选择，不能创建独立窗口"));
    h.click(H::local(343.0,46.0));
    for _ in 0..90 {h.frame(vec![]);}
    assert_eq!(h.app.expanded_height(),PH,"右侧滚轮应保持待办页原来的高度");
    assert!(h.hit.height()<=PH+2.0,"选择时间不能撑大岛体");
    for label in ["准备会议","已完成","取消","完成","暂不提醒"] {
        let (rect,clip)=h.shapes.iter().find_map(|s|match &s.shape {
            egui::Shape::Text(t) if t.galley.text()==label=>Some((t.visual_bounding_rect(),s.clip_rect)),_=>None,
        }).unwrap_or_else(||panic!("岛内缺少选项：{label}"));
        assert!(h.hit.contains_rect(rect),"选项 {label} 应在岛体命中区域内");
        assert!(h.region.contains_rect(rect) && clip.contains_rect(rect),"选项 {label} 不能被窗口或岛体裁剪");
    }
    h.frame(vec![egui::Event::Key{key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);
    assert!(h.app.task_dialog.is_none());assert!(h.app.todos.items[0].due.is_none());
}

#[test]
fn reminder_wheels_select_date_hour_and_minute_without_typing(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let day=chrono::Local::now().date_naive()+chrono::Duration::days(2);let original=format!("{day} 09:00");
    h.app.todos.items=vec![crate::model::TodoItem{text:"准备方案".into(),due:Some(original.clone()),done_on:None,reminded:false,..Default::default()}];
    h.app.open_task_reminder(0);
    for x in [235.0,290.0,346.0] {h.hover(H::local(x,77.0),0.4);h.wheel(-40.0);}
    assert_eq!(h.app.todos.items[0].due,Some(original),"调整时不能提前写入提醒");
    h.picker_click("完成");
    let next=day+chrono::Duration::days(1);
    assert_eq!(h.app.todos.items[0].due,Some(format!("{next} 10:01")));
    assert!(h.app.task_dialog.is_none());
}

#[test]
fn reminder_wheels_support_clicks_and_dragging(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let day=chrono::Local::now().date_naive()+chrono::Duration::days(2);
    h.app.todos.items=vec![crate::model::TodoItem{text:"确认方案".into(),due:Some(format!("{day} 10:30")),done_on:None,reminded:false,..Default::default()}];
    h.app.open_task_reminder(0);h.hover(H::local(300.0,60.0),1.0);
    h.click(H::local(346.0,96.0));h.click(H::local(290.0,58.0));
    let start=H::local(235.0,77.0);h.cursor=Some(start);
    h.frame(vec![egui::Event::PointerMoved(start),egui::Event::PointerButton{pos:start,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
    for y in [59.0,41.0]{let point=H::local(235.0,y);h.cursor=Some(point);h.frame(vec![egui::Event::PointerMoved(point)]);}
    let end=H::local(235.0,41.0);
    h.frame(vec![egui::Event::PointerButton{pos:end,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
    h.picker_click("完成");
    assert_eq!(h.app.todos.items[0].due,Some(format!("{} 09:31",day+chrono::Duration::days(2))));
    assert!(h.app.todos.items[0].done_on.is_none());
}

#[test]
fn reminder_wheels_accumulate_small_trackpad_movements(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let day=chrono::Local::now().date_naive()+chrono::Duration::days(2);
    h.app.todos.items=vec![crate::model::TodoItem{text:"检查进度".into(),due:Some(format!("{day} 10:30")),done_on:None,reminded:false,..Default::default()}];
    h.app.open_task_reminder(0);h.hover(H::local(346.0,77.0),1.0);
    for _ in 0..80{h.wheel(-0.5);}
    h.picker_click("完成");
    assert_eq!(h.app.todos.items[0].due,Some(format!("{day} 10:31")));
    assert_eq!(h.app.page(),4,"操作时间滚轮不能触发翻页");
}

#[test]
fn reminder_picker_rejects_past_time_and_cancel_preserves_the_task(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let old=format!("{} 09:00",chrono::Local::now().date_naive()-chrono::Duration::days(1));
    h.app.todos.items=vec![crate::model::TodoItem{text:"保留任务".into(),due:Some(old.clone()),done_on:None,reminded:true,..Default::default()}];
    h.app.open_task_reminder(0);h.picker_click("完成");h.picker_frame(vec![]);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()=="请选择晚于当前的时间")));
    assert_eq!(h.app.todos.items[0].due,Some(old.clone()));
    h.picker_click("取消");assert!(h.app.task_dialog.is_none());assert_eq!(h.app.todos.items[0].due,Some(old));
}










#[test]
fn remove_icons_are_centered_in_their_buttons_at_each_scale(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for scale in [1.0,1.25,1.5,2.0]{
        let mut h=H::new();h.ctx.set_pixels_per_point(scale);
        h.app.todos.items=["第一项","第二项","第三项"].into_iter().map(|text|crate::model::TodoItem{text:text.into(),due:None,done_on:None,reminded:false,..Default::default()}).collect();
        h.app.show_page(4);h.hover(H::local(300.0,60.0),1.0);
        for i in 0..3usize{
            let button=h.ctx.read_response(egui::Id::new(("card",true,("todo-delete",i)))).unwrap().rect;
            let ink=h.shapes.iter().filter_map(|s|match &s.shape{
                egui::Shape::Text(t) if t.galley.text()=="×" && button.contains(t.visual_bounding_rect().center())=>Some(t.visual_bounding_rect()),
                egui::Shape::LineSegment{points,..} if points.iter().all(|p|button.shrink(2.0).contains(*p)) && (points[0].x-points[1].x).abs()>2.0 && (points[0].y-points[1].y).abs()>2.0=>Some(Rect::from_two_pos(points[0],points[1])),
                _=>None,
            }).fold(Rect::NOTHING,|a,b|a.union(b));
            assert!(ink.is_positive(),"删除按钮应有可见图案");
            assert!(ink.center().distance(button.center())<0.4,"缩放 {scale}：删除图案中心 {:?} 偏离按钮中心 {:?}",ink.center(),button.center());
        }
        h.click(H::local(361.5,68.0));
        assert_eq!(h.app.todos.items.iter().map(|t|t.text.as_str()).collect::<Vec<_>>(),["第一项","第三项"]);
        assert!(h.app.todos.items.iter().all(|t|t.done_on.is_none()),"删除入口不能同时勾选其他待办");
    }
}




#[test]
fn shelf_remove_icon_stays_centered_on_hover_and_preserves_the_file(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for ppp in [1.0,1.25,1.5,2.0]{
        let mut h=H::new();h.ctx.set_pixels_per_point(ppp);
        let path=crate::model::config_dir().join("设计说明.txt");std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(&path,"保留原文件").unwrap();
        h.app.shelf.items=vec![path.clone()];h.app.show_page(3);
        for pointer in [H::local(300.0,60.0),H::local(233.0,39.0)]{
            h.hover(pointer,1.0);
            let button=h.ctx.read_response(egui::Id::new(("card",true,("shelf-remove",&path)))).unwrap().rect;
            let ink=h.shapes.iter().filter_map(|s|match &s.shape{
                egui::Shape::LineSegment{points,..} if points.iter().all(|p|button.shrink(2.0).contains(*p)) && (points[0].x-points[1].x).abs()>2.0 && (points[0].y-points[1].y).abs()>2.0=>Some(Rect::from_two_pos(points[0],points[1])),_=>None,
            }).fold(Rect::NOTHING,|a,b|a.union(b));
            assert!(ink.is_positive() && ink.center().distance(button.center())<0.4,"置物架移除图案应与按钮同心");
        }
        h.click(H::local(233.0,39.0));assert!(h.app.shelf.items.is_empty());assert!(path.exists());
    }
}

#[test]
fn resident_components_can_be_dragged_saved_and_reset(){
    use crate::bar_layout::{CLOCK,BATTERY,WEATHER};
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    fn draw(h:&mut H,events:Vec<egui::Event>,slot:usize)->[Pos2;2]{
        let mut points=[Pos2::ZERO;2];
        h.t+=0.02;
        let out=h.ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(600.0,300.0))),time:Some(h.t),events,..Default::default()},|ctx|{egui::CentralPanel::default().show(ctx,|ui|{h.app.resident_order_settings(ui);for (index,slot) in [slot,BATTERY].into_iter().enumerate(){points[index]=ui.ctx().read_response(ui.id().with(("resident-icon",slot))).expect("暂未显示的组件也应可以独立拖动").rect.center();}});});
        h.shapes=out.shapes;points
    }
    fn point(h:&H,label:&str)->Pos2{h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t) if t.galley.text()==label=>Some(t.visual_bounding_rect().center()),_=>None}).unwrap()}
    for slot in [CLOCK,WEATHER] {
    let mut h=H::new();
    draw(&mut h,vec![],slot);let [from,to]=draw(&mut h,vec![],slot);
    let button=|pos,pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()};
    draw(&mut h,vec![egui::Event::PointerMoved(from),button(from,true)],slot);
    draw(&mut h,vec![egui::Event::PointerMoved(from+egui::vec2(0.0,10.0))],slot);
    draw(&mut h,vec![egui::Event::PointerMoved(to)],slot);
    for _ in 0..3{draw(&mut h,vec![],slot);}
    draw(&mut h,vec![button(to,false)],slot);
    assert_eq!(h.app.settings.resident_order.first(),Some(&slot));
    assert!(h.app.settings.resident_order.contains(&BATTERY));
    h.app.settings.save();assert_eq!(crate::model::Settings::load().resident_order,h.app.settings.resident_order);
    let reset=point(&h,"恢复默认");draw(&mut h,vec![egui::Event::PointerMoved(reset),button(reset,true)],slot);draw(&mut h,vec![button(reset,false)],slot);
    assert!(h.app.settings.resident_order.is_empty());
    }
}


fn draw_page_management(h:&mut H,events:Vec<egui::Event>){
    let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();h.t+=0.02;let time=h.t;
    egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
        let id=viewport.ids.this;
        let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,1500.0))),time:Some(time),events:events.clone(),viewports:std::iter::once((id,egui::ViewportInfo::default())).collect(),..Default::default()};
        *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
    });
    let _=h.ctx.run(Default::default(),|ctx|h.app.settings_ui(ctx,false));
    h.shapes=output.lock().unwrap().take().unwrap().shapes;
}

#[cfg(feature = "material")]
#[test]
fn page_management_page_shortcuts_use_plugin_controls() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    install_page_shortcuts_fixture(&mut h);
    h.app.plugins.enable(SHORTCUT_FIXTURE_ID,true).unwrap();
    h.app.settings.settings_theme=crate::settings_theme::SettingsTheme::Dark;
    h.app.settings_tab=crate::extras::SETTINGS_TAB_PAGES;
    h.app.settings.show_page_shortcuts=true;
    h.app.open_settings();
    h.ctx.set_embed_viewports(false);
    for _ in 0..12 {draw_page_management(&mut h,vec![]);}
    let title=h.shapes.iter().find_map(|shape|match &shape.shape {
        egui::Shape::Text(text) if text.galley.text()=="快捷切换图标"=>Some(text.visual_bounding_rect()),
        _=>None,
    }).expect("页面管理应显示已启用插件的图标设置");
    assert!(h.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text)
        if text.galley.text()=="插件" && text.visual_bounding_rect().left()>=title.right()
            && (text.visual_bounding_rect().center().y-title.center().y).abs()<8.0)),
        "快捷切换图标标题旁应有插件小标");
    let pos=h.shapes.iter().find_map(|shape|match &shape.shape {
        egui::Shape::Rect(rect) if rect.fill==egui::Color32::from_rgb(140,127,208)
            && (rect.rect.center().y-title.center().y).abs()<24.0 => Some(rect.rect.center()),
        _=>None,
    }).expect("快捷切换图标开启时应用紫色开关");
    for pressed in [true,false] {
        draw_page_management(&mut h,vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{
            pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default(),
        }]);
    }
    assert!(!h.app.settings.show_page_shortcuts);
    assert!(h.app.plugins.active_page_shortcut_colors().is_some(),"隐藏图标时插件仍启用");
}

#[test]
fn weather_status_switch_controls_the_badge_and_keeps_the_page_available(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for theme in [crate::settings_theme::SettingsTheme::Dark,crate::settings_theme::SettingsTheme::Light] {
        let mut h=H::new();h.app.settings.settings_theme=theme;h.app.settings_tab=10;h.app.open_settings();h.ctx.set_embed_viewports(false);
        h.app.weather.snapshot=Some(crate::weather::Snapshot{temp:9.0,..Default::default()});
        for _ in 0..3 {draw_page_management(&mut h,vec![]);}
        let text_rect=|h:&H,label:&str|h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t) if t.galley.text()==label=>Some(t.visual_bounding_rect()),_=>None}).unwrap();
        let heading=text_rect(&h,"岛上显示");let bottom=text_rect(&h,"功能页面").top();
        let title=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t) if t.galley.text()=="天气"&&t.pos.x>=heading.left()&&t.pos.y>heading.bottom()&&t.pos.y<bottom=>Some(t.visual_bounding_rect()),_=>None}).expect("岛上显示应有独立的天气开关");
        let point=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Rect(r) if (r.rect.width()-34.0).abs()<0.01&&(r.rect.height()-20.0).abs()<0.01&&r.rect.center().y>title.top()&&r.rect.center().y<title.bottom()+24.0=>Some(r.rect.center()),_=>None}).unwrap();
        for expected in [false,true] {
            for pressed in [true,false] {draw_page_management(&mut h,vec![egui::Event::PointerMoved(point),egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}]);}
            assert_eq!(h.app.weather_pill().is_some(),expected);
            assert_eq!(h.app.side_items().contains(&crate::side_dock::SideItem::Weather),expected);
            assert!(h.app.module_on(8),"关闭状态组件后天气页仍可使用");
            h.app.settings=crate::model::Settings::load();assert_eq!(h.app.weather_pill().is_some(),expected,"天气开关应自动保存");
        }
    }
}





#[test]
fn hidden_components_do_not_keep_static_pages_animating(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.settings.pet=false;
    h.app.show_page(4);h.hover(H::local(300.0,60.0),2.0);
    assert!(!h.animating);
    let mut moving=crate::anim::Spring::new(0.0,230.0,0.9);moving.target=200.0;
    h.app.shelf_x.insert("hidden.txt".into(),moving);
    h.frame(vec![]);
    assert!(!h.animating,"离开置物页后，未完成的文件动画不能让其他页面持续刷新");
    h.app.shelf_x.clear();h.app.pomo.running=true;
    h.hover(H::local(300.0,60.0),1.5);
    assert!(!h.animating,"待办页没有显示专注动画，计时继续但不应强制连续刷新");
}


#[test]
fn native_app_icon_uses_bounded_resolution(){
    let icon=crate::theme::app_icon();
    assert!(icon.width<=128 && icon.height<=128,"原生窗口图标不需要常驻完整的 1024 像素品牌原图");
    assert_eq!(icon.rgba.len(),(icon.width*icon.height*4)as usize);
    assert!(std::sync::Arc::ptr_eq(&icon,&crate::theme::app_icon()));
}







#[test]
fn page_order_drag_applies_on_release_and_saves() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.settings_tab=10;h.app.open_settings();h.ctx.set_embed_viewports(false);
    fn draw(h:&mut H,events:Vec<egui::Event>){
        let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();
        h.t+=0.02;let time=h.t;
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,1200.0))),time:Some(time),events:events.clone(),viewports:std::iter::once((id,egui::ViewportInfo::default())).collect(),..Default::default()};
            *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        let _=h.ctx.run(egui::RawInput::default(),|ctx|h.app.settings_ui(ctx,false));
        let child=output.lock().unwrap().take().unwrap();h.shapes=child.shapes;
    }
    for _ in 0..3 {draw(&mut h,vec![]);}
    let point=|h:&H,label:&str|h.shapes.iter().find_map(|s|match &s.shape {egui::Shape::Text(t) if t.galley.text()==label && s.clip_rect.contains(t.visual_bounding_rect().left_center())=>Some(t.visual_bounding_rect().left_center()),_=>None}).unwrap();
    let from=point(&h,"临时收纳文件位置")+egui::vec2(-18.0,-6.0);
    let to=point(&h,"喝水与久坐提醒");
    let button=|pos,pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()};
    draw(&mut h,vec![egui::Event::PointerMoved(from),button(from,true)]);
    draw(&mut h,vec![egui::Event::PointerMoved(from+egui::vec2(0.0,10.0))]);
    draw(&mut h,vec![egui::Event::PointerMoved(to)]);
    assert_eq!(h.app.settings.page_order,[0,1,2,3,4,5,6,7,8,9,10]);
    draw(&mut h,vec![button(to,false)]);
    assert_eq!(h.app.settings.page_order,[0,1,3,2,4,5,6,7,8,9,10]);
    assert_eq!(crate::model::Settings::load().page_order,h.app.settings.page_order);
}

#[test]
fn general_settings_controls_fit_when_resizing_to_minimum_width() {
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.open_settings();
    h.app.settings.skin=crate::model::Skin::GlassDark;
    h.ctx.set_embed_viewports(false);
    for (width,scale) in [(800.0,1.0),(600.0,2.0),(690.0,1.25)] {
        let output=std::sync::Arc::new(Mutex::new(None));
        let capture=output.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            let input=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(width,1800.0))),
                viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(scale),..Default::default()})).collect(),..Default::default()};
            *capture.lock().unwrap()=Some(ctx.run(input,|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        // 皮肤与大小在「外观」，位置微调在「位置」。
        for (tab,labels) in [(crate::extras::SETTINGS_TAB_APPEARANCE,vec!["经典","奶油","毛玻璃·浅","毛玻璃·深","悬浮云","恢复默认"]),(crate::extras::SETTINGS_TAB_POSITION,vec!["恢复居中"])] {
        h.app.settings_tab=tab;
        for _ in 0..3 {let _=h.ctx.run(Default::default(),|ctx|h.app.settings_ui(ctx,false));}
        let frame=output.lock().unwrap().take().unwrap();
        for label in labels {
            let shape=frame.shapes.iter().find(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==label))
                .unwrap_or_else(||panic!("窗口宽 {width} 时缺少控件 {label}"));
            assert!(shape.clip_rect.contains_rect(shape.shape.visual_bounding_rect()),"窗口宽 {width} 时 {label} 被裁切");
        }
        for shape in &frame.shapes {
            if let egui::Shape::Rect(r)=&shape.shape {
                if r.rect.size()==egui::vec2(34.0,20.0) {
                    assert!(r.rect.right()<=shape.clip_rect.right(),"窗口宽 {width} 时开关右侧被裁切");
                }
            }
        }
        }
    }
}


#[test]
#[ignore = "按需导出关于页支持作者预览"]
fn about_support_preview() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_SUPPORT_PREVIEW_DIR").expect("需指定预览目录"));
    std::fs::create_dir_all(&dest).unwrap();
    let mut h=H::new();h.app.open_settings();h.app.settings_tab=9;h.ctx.set_embed_viewports(false);
    let output=std::sync::Arc::new(std::sync::Mutex::new(None));let capture=output.clone();
    egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
        let id=viewport.ids.this;
        let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,560.0))),
            viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()};
        *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
    });
    let mut raster=crate::preview_raster::Raster::new(690,560);
    for dark in [true,false] { for (mode,open,wall) in [("closed",false,false),("open",true,false),("wall",false,true)] {
        h.app.settings.settings_theme=if dark{crate::settings_theme::SettingsTheme::Dark}else{crate::settings_theme::SettingsTheme::Light};
        h.ctx.data_mut(|data|{
            data.insert_temp(egui::Id::new("support-author-open"),open);
            data.insert_temp(egui::Id::new("support-author-wall"),wall);
        });
        for frame in 0..4 {
            let root=h.ctx.run(Default::default(),|ctx|h.app.settings_ui(ctx,false));
            let child=output.lock().unwrap().take().unwrap();
            raster.update_textures(&root.textures_delta);raster.update_textures(&child.textures_delta);
            if frame==3 {
                raster.buf.fill([0.0,0.0,0.0,1.0]);
                raster.draw(&h.ctx.tessellate(child.shapes.clone(),child.pixels_per_point),child.pixels_per_point);
                let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
                image::save_buffer(dest.join(format!("about-{}-{mode}.png",if dark{"dark"}else{"light"})),&rgba,690,560,image::ColorType::Rgba8).unwrap();
                let has_wechat=child.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()=="微信支付"));
                assert_eq!(has_wechat,open);
                if wall {
                    assert!(child.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Rect(image)
                        if image.brush.is_some() && image.rect.width()>350.0 && image.rect.height()>450.0)),
                        "感谢墙应撑满可用宽度，超出窗口的部分可滚动查看");
                }
                let labels: &[&str] = match mode {
                    "closed" => &["请喝咖啡 · 支持作者", "完全自愿 · 感谢每一份支持 · 不影响功能使用", "最近支持", "查看感谢墙 →"],
                    "open" => &["返回", "完全自愿 · 感谢每一份支持 · 不影响功能使用"],
                    _ => &["感谢墙", "← 返回", "按累计支持排序"],
                };
                for &label in labels {
                    let shape=child.shapes.iter().find(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()==label)).unwrap_or_else(||panic!("{mode} 状态的控件必须可见：{label}"));
                    assert!(shape.clip_rect.contains_rect(shape.shape.visual_bounding_rect()),"默认窗口中内容不能被裁切：{label}");
                }
            }
            raster.free_textures(&root.textures_delta);raster.free_textures(&child.textures_delta);
        }
    } }
}






#[test]
fn clock_seconds_formats_and_persists(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();assert!(!h.app.settings.clock_seconds);
    assert_eq!(h.app.settings.clock_text("23:59:58"),"23:59");
    h.app.settings.clock_seconds=true;
    for (input,expected) in [("00:00:00","00:00:00"),("23:59:59","23:59:59"),("13:05","13:05:00"),("invalid","--:--:--")] {
        assert_eq!(h.app.settings.clock_text(input),expected);
    }
    h.app.settings.clock_12h=true;h.app.settings.clock_period=1;
    for (input,expected) in [("00:00:01","上午 12:00:01"),("12:00:00","下午 12:00:00"),("13:05:08","下午 1:05:08")] {
        assert_eq!(h.app.settings.clock_text(input),expected);
    }
    h.app.settings.clock_period=2;
    assert_eq!(h.app.settings.clock_text("23:59:59"),"晚上 11:59:59");
    for enabled in [true,false] {
        h.app.settings.clock_seconds=enabled;h.app.settings.save();
        assert_eq!(crate::model::Settings::load().clock_seconds,enabled);
    }
}


#[test]
fn clock_residency_switch_in_page_management_saves(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    fn draw(h:&mut H,raster:&mut crate::preview_raster::Raster,events:Vec<egui::Event>){
        let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();h.t+=0.02;let time=h.t;
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            *capture.lock().unwrap()=Some(ctx.run(egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(780.0,1400.0))),time:Some(time),events:events.clone(),viewports:std::iter::once((id,egui::ViewportInfo::default())).collect(),..Default::default()},|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        let root=h.ctx.run(Default::default(),|ctx|h.app.settings_ui(ctx,false));let child=output.lock().unwrap().take().unwrap();
        raster.update_textures(&root.textures_delta);raster.update_textures(&child.textures_delta);h.shapes=child.shapes;
    }
    for dark in [true,false] {
        let mut h=H::new();h.app.settings.settings_theme=if dark{crate::settings_theme::SettingsTheme::Dark}else{crate::settings_theme::SettingsTheme::Light};
        h.app.settings_tab=10;h.app.open_settings();h.ctx.set_embed_viewports(false);
        let mut raster=crate::preview_raster::Raster::new(780,1400);
        assert!(h.app.settings.show_clock,"已有配置默认保持时间常驻");
        for _ in 0..3{draw(&mut h,&mut raster,vec![]);}
        let title=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()=="时间"=>Some(t.visual_bounding_rect()),_=>None}).expect("状态组件应提供时间开关");
        let point=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Rect(r)if (r.rect.width()-34.0).abs()<0.01&&(r.rect.height()-20.0).abs()<0.01&&r.rect.center().y>title.top()&&r.rect.center().y<title.bottom()+24.0=>Some(r.rect.center()),_=>None}).unwrap();
        for expected in [false,true] {
            for pressed in [true,false]{draw(&mut h,&mut raster,vec![egui::Event::PointerMoved(point),egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}]);}
            draw(&mut h,&mut raster,vec![]);
            assert_eq!(h.app.settings.show_clock,expected);assert_eq!(crate::model::Settings::load().show_clock,expected,"常驻时间开关应自动保存");
            let preview=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text().contains(':')&&t.galley.job.sections.first().is_some_and(|s|(s.format.font_id.size-crate::theme::size::PILL_CLOCK).abs()<0.01)=>Some(t.fallback_color.a()),_=>None}).expect("常驻排列应保留时间的位置");
            assert_eq!(preview==255,expected,"关闭时间后排列预览应置灰");
            if let Some(dest)=std::env::var_os("LINGQI_CLOCK_RESIDENCY_PREVIEW") {
                let dest=std::path::PathBuf::from(dest);std::fs::create_dir_all(&dest).unwrap();
                raster.buf.fill([0.08,0.08,0.09,1.0]);raster.draw(&h.ctx.tessellate(h.shapes.clone(),1.0),1.0);let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
                image::save_buffer(dest.join(format!("settings-{}-{}.png",if dark{"dark"}else{"light"},if expected{"on"}else{"off"})),&rgba,780,1400,image::ColorType::Rgba8).unwrap();
            }
        }
    }
}


#[test]
fn clock_seconds_setting_toggles_and_saves(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();h.app.open_settings();h.ctx.set_embed_viewports(false);
    let mut raster=crate::preview_raster::Raster::new(780,1800);
    fn frame(h:&mut H,raster:&mut crate::preview_raster::Raster,events:Vec<egui::Event>){
        h.t+=1.0/60.0;let time=h.t;
        let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            *capture.lock().unwrap()=Some(ctx.run(egui::RawInput{time:Some(time),viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(780.0,1800.0))),events:events.clone(),viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()},|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        let root=h.ctx.run(egui::RawInput{time:Some(time),..Default::default()},|ctx|h.app.settings_ui(ctx,false));let child=output.lock().unwrap().take().unwrap();
        raster.update_textures(&root.textures_delta);raster.update_textures(&child.textures_delta);h.shapes=child.shapes;
    }
    for _ in 0..3{frame(&mut h,&mut raster,vec![]);}
    let label=|h:&H|h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()=="显示秒数"&&s.clip_rect.contains_rect(t.visual_bounding_rect())=>Some(t.visual_bounding_rect()),_=>None});
    assert!(!h.app.settings.clock_seconds);
    for enabled in [true,false] {
        let title=label(&h).expect("常规设置应提供显示秒数开关");
        let toggle=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Rect(r)if (r.rect.width()-34.0).abs()<0.1&&(r.rect.height()-20.0).abs()<0.1&&(r.rect.top()-title.top()).abs()<18.0=>Some(r.rect.center()),_=>None}).expect("秒数开关应与文字对齐");
        for pressed in [true,false]{frame(&mut h,&mut raster,vec![egui::Event::PointerMoved(toggle),egui::Event::PointerButton{pos:toggle,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()}]);}
        frame(&mut h,&mut raster,vec![]);
        assert_eq!(h.app.settings.clock_seconds,enabled);assert_eq!(crate::model::Settings::load().clock_seconds,enabled);
        if let Some(dest)=std::env::var_os("LINGQI_CLOCK_PREVIEW_DIR") {
            let dest=std::path::PathBuf::from(dest);std::fs::create_dir_all(&dest).unwrap();
            raster.buf.fill([0.08,0.08,0.09,1.0]);raster.draw(&h.ctx.tessellate(h.shapes.clone(),1.0),1.0);let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
            image::save_buffer(dest.join(if enabled{"settings-on.png"}else{"settings-off.png"}),&rgba,780,1800,image::ColorType::Rgba8).unwrap();
        }
    }
}


#[test]
fn clock_period_boundaries_and_both_formats() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    for twelve in [false,true] {
        h.app.settings.clock_12h=twelve;
        h.app.settings.clock_period=2;
        for (time,label) in [("00:00","凌晨"),("05:59","凌晨"),("06:00","上午"),("10:59","上午"),("11:00","中午"),("12:59","中午"),("13:00","下午"),("16:59","下午"),("17:00","傍晚"),("18:59","傍晚"),("19:00","晚上"),("23:59","晚上")] {
            assert!(h.app.settings.clock_text(time).starts_with(&format!("{label} ")));
        }
        assert_eq!(h.app.settings.clock_text("19:30"),if twelve {"晚上 7:30"}else{"晚上 19:30"});
        for mode in 0..=2 {
            h.app.settings.clock_period=mode;
            h.app.settings.save();
            let loaded=crate::model::Settings::load();
            assert_eq!(loaded.clock_period,mode);
            assert_eq!(loaded.clock_12h,twelve);
        }
    }
}

#[test]
fn clock_period_visually_centers_with_digits() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let h=H::new();
    let mut raster=crate::preview_raster::Raster::new(420,230);
    for scale in [1.0,1.25,1.5,2.0] {
        let output=h.ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(420.0,230.0))),..Default::default()},|ctx|{
            egui::CentralPanel::default().show(ctx,|ui|{
                let mut c=crate::paint::Card::new(ui,Pos2::ZERO,ui.max_rect());c.s=scale;
                for (i,period) in ["凌晨","上午","中午","下午","傍晚","晚上"].iter().enumerate() {
                    for (j,size) in [12.5,15.5].iter().enumerate() {
                        crate::paint::clock(&c,10.0+j as f32*100.0,18.0+i as f32*18.0,&format!("{period} 12:59"),*size);
                    }
                }
            });
        });
        let texts:Vec<_>=output.shapes.iter().filter_map(|s|match &s.shape{egui::Shape::Text(t)=>Some(t),_=>None}).collect();
        for pair in texts.chunks_exact(2) {
            assert!((pair[0].visual_bounding_rect().center().y-pair[1].visual_bounding_rect().center().y).abs()<0.1,"时段文字与数字应按可见字形垂直居中，缩放 {scale}");
        }
        raster.update_textures(&output.textures_delta);
        if scale==2.0 {
            if let Some(dest)=std::env::var_os("LINGQI_CLOCK_ALIGNMENT_PREVIEW") {
                raster.buf.fill([0.0,0.0,0.0,1.0]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
                let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
                image::save_buffer(dest,&rgba,420,230,image::ColorType::Rgba8).unwrap();
            }
        }
    }
}

#[test]
fn avoidance_picker_add_remove_and_save() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.ctx.set_visuals(egui::Visuals::dark());
    assert!(h.app.settings.avoid_browsers);
    assert!(h.app.settings.avoid_apps.is_empty());
    h.app.settings.browser_minimal=true;
    h.app.avoidance.entries=vec![
        crate::avoidance::Candidate{path:r"D:\Quark\quark.exe".into(),name:"夸克浏览器".into(),title:"新标签页".into(),browser:true},
        crate::avoidance::Candidate{path:r"C:\Windows\System32\notepad.exe".into(),name:"记事本".into(),title:"未命名 - 记事本".into(),browser:false},
    ];
    fn draw(h:&mut H,events:Vec<egui::Event>)->egui::FullOutput {
        h.t+=1.0/60.0;
        h.ctx.run(egui::RawInput{time:Some(h.t),screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(600.0,560.0))),events,..Default::default()},|ctx|{
            egui::CentralPanel::default().show(ctx,|ui|{
                ui.heading("常规");
                h.app.avoidance.settings(ui,&mut h.app.settings);
            });
            h.app.avoidance.dialog(ctx,&mut h.app.settings);
        })
    }
    fn click(h:&mut H,point:Pos2){
        draw(h,vec![egui::Event::PointerMoved(point),egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
        draw(h,vec![egui::Event::PointerMoved(point),egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
    }
    fn point(out:&egui::FullOutput,label:&str)->Pos2 {
        out.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t) if t.galley.text()==label=>Some(t.visual_bounding_rect().center()),_=>None}).unwrap_or_else(||panic!("未找到 {label}"))
    }
    h.app.avoidance.open=true;
    let mut raster=crate::preview_raster::Raster::new(600,560);
    let mut out=draw(&mut h,vec![]);raster.update_textures(&out.textures_delta);
    for _ in 0..40 {std::thread::sleep(std::time::Duration::from_millis(10));out=draw(&mut h,vec![]);raster.update_textures(&out.textures_delta);}
    assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="浏览器规则已覆盖")));
    if let Some(dest)=std::env::var_os("LINGQI_AVOID_PREVIEW_DIR") {
        std::fs::create_dir_all(&dest).unwrap();raster.buf.fill([0.0,0.0,0.0,1.0]);
        raster.draw(&h.ctx.tessellate(out.shapes.clone(),out.pixels_per_point),out.pixels_per_point);
        let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);image::save_buffer(std::path::Path::new(&dest).join("添加应用.png"),&rgba,600,560,image::ColorType::Rgba8).unwrap();
    }
    click(&mut h,point(&out,"记事本"));
    let out=draw(&mut h,vec![]);
    click(&mut h,point(&out,"添加"));
    assert!(!h.app.avoidance.open);
    assert_eq!(h.app.settings.avoid_apps.len(),1);
    h.app.settings.avoid_browsers=false;h.app.settings.save();
    let loaded=crate::model::Settings::load();
    assert!(!loaded.avoid_browsers);assert!(loaded.browser_minimal);
    assert_eq!(loaded.avoid_apps,h.app.settings.avoid_apps);
    let mut out=draw(&mut h,vec![]);raster.update_textures(&out.textures_delta);
    for _ in 0..40{std::thread::sleep(std::time::Duration::from_millis(20));out=draw(&mut h,vec![]);raster.update_textures(&out.textures_delta);}
    assert!(!out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="记事本")),"图标区不应常驻显示应用名称");
    if let Some(dest)=std::env::var_os("LINGQI_AVOID_PREVIEW_DIR") {
        raster.buf.fill([0.0,0.0,0.0,1.0]);raster.draw(&h.ctx.tessellate(out.shapes.clone(),out.pixels_per_point),out.pixels_per_point);
        let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);image::save_buffer(std::path::Path::new(&dest).join("图标区域.png"),&rgba,600,560,image::ColorType::Rgba8).unwrap();
    }
    click(&mut h,point(&out,"×"));assert!(h.app.settings.avoid_apps.is_empty());
    h.app.settings.save();assert!(crate::model::Settings::load().avoid_apps.is_empty());

    h.app.settings.avoid_browsers=true;
    h.app.settings.avoid_apps.push(("D:/one.exe".into(),"已登记".into()));
    for (path,name,browser) in [("d:/ONE.exe","已登记",false),("D:/two.exe","工具二",false),
        ("D:/three.exe","工具三",false),("D:/browser.exe","浏览器",true)] {
        h.app.avoidance.entries.push(crate::avoidance::Candidate{path:path.into(),name:name.into(),title:"插件面板".into(),browser});
    }
    h.app.avoidance.open=true;
    draw(&mut h,vec![]);
    let out=draw(&mut h,vec![]);
    click(&mut h,point(&out,"搜索应用或窗口标题"));
    draw(&mut h,vec![egui::Event::Text("插件面板".into())]);
    let out=draw(&mut h,vec![]);
    click(&mut h,point(&out,"全部添加"));
    // 全部添加需要二次确认：第一次只出现确认条，不添加。
    assert!(h.app.avoidance.open);assert_eq!(h.app.settings.avoid_apps.len(),1,"确认前不能添加");
    let out=draw(&mut h,vec![]);
    click(&mut h,point(&out,"确定添加"));
    assert!(!h.app.avoidance.open);
    assert_eq!(h.app.settings.avoid_apps.iter().map(|(p,_)|p.as_str()).collect::<Vec<_>>(),["D:/one.exe","D:/two.exe","D:/three.exe"]);
    h.app.settings.save();assert_eq!(crate::model::Settings::load().avoid_apps,h.app.settings.avoid_apps);
}

#[test]
fn six_minutes_without_interaction_do_not_restart_static_animation_or_grow_textures(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.settings.pet=false;h.app.water.remind=false;
    for _ in 0..120{h.frame(vec![]);}
    let textures=h.ctx.tex_manager().read().num_allocated();
    for second in 3..=363 {
        h.t=second as f64;h.system_idle=Some(second as f64);h.frame(vec![]);
        assert!(!h.animating,"空闲到第 {second} 秒不应重新进入连续动画");
        assert_eq!(h.ctx.tex_manager().read().num_allocated(),textures,"空闲纹理数不能持续增长");
    }
}

#[test]
fn avoidance_icon_area_stays_one_row_and_scrolls() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.ctx.set_visuals(egui::Visuals::dark());h.app.settings.browser_minimal=true;
    fn draw(h:&mut H,events:Vec<egui::Event>)->egui::FullOutput {
        h.t+=1.0/60.0;
        h.ctx.run(egui::RawInput{time:Some(h.t),screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(420.0,560.0))),events,..Default::default()},|ctx|{
            egui::CentralPanel::default().show(ctx,|ui|{h.app.avoidance.settings(ui,&mut h.app.settings);});
        })
    }
    fn text(out:&egui::FullOutput,label:&str)->Rect {
        out.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t) if t.galley.text()==label=>Some(t.visual_bounding_rect()),_=>None}).unwrap()
    }
    let footer="切到插件浮窗时，宿主窗口仍遮住岛就会继续避让。";
    h.app.settings.avoid_apps=vec![("D:/sample/A.exe".into(),"A应用".into())];
    draw(&mut h,vec![]);let one=draw(&mut h,vec![]);let baseline=text(&one,footer).top();
    let cross=text(&one,"×").center();
    assert!(one.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Circle(c) if c.radius<=7.1 && c.center.distance(cross)<3.0)),"移除角标应为小圆点，不能被按钮最小尺寸撑大");
    h.app.settings.avoid_apps=(0..30).map(|i|(format!("D:/sample/{i}.exe"),format!("{}应用",char::from_u32(65+i).unwrap()))).collect();
    draw(&mut h,vec![]);let many=draw(&mut h,vec![]);
    assert!((text(&many,footer).top()-baseline).abs()<1.0,"应用增多时始终保持单行高度");
    let first=text(&many,"A");let pointer=first.center();
    draw(&mut h,vec![egui::Event::PointerMoved(pointer),egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:egui::vec2(0.0,-150.0),modifiers:Default::default()}]);
    let mut scrolled=draw(&mut h,vec![]);for _ in 0..30{scrolled=draw(&mut h,vec![]);}
    assert!((text(&scrolled,footer).top()-text(&many,footer).top()).abs()<1.0,"滚动图标不能移动区域下方内容");
    assert!(scrolled.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="A" && t.visual_bounding_rect().left()<first.left()-10.0)) || !scrolled.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="A")),"应能滚动查看后续应用");
}

#[test]
fn compact_canvas_preserves_panel_and_notice_pixels(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    fn draw(width:f32,page:Option<usize>,notice:bool)->Vec<u8>{
        let mut h=H::new();
        if let Some(page)=page{h.app.show_page(page);}
        let mut raster=crate::preview_raster::Raster::new(1280,270);
        let mut rgba=Vec::new();
        for frame in 0..180 {
            if notice && frame==60{h.app.alert("收到新邮件","会议通知及验证码 834921",5,true);}
            let click=(notice && h.app.compact_notices() && (frame==90 || frame==91)).then(||h.notice_button().center());
            let t=frame as f64/60.0;
            let env=Env{t,clock:"09:41:00".into(),today:"2026-10-28".into(),date:"10月28日 周三".into(),cursor:click.or_else(||page.map(|_|H::local(300.0,40.0))),..Default::default()};
            let output=h.ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(width,WIN_H))),time:Some(t),events:click.map(|pos|vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:frame==90,modifiers:Default::default()}]).unwrap_or_default(),..Default::default()},|ctx|{
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{h.app.frame(ui,Rect::from_min_size(Pos2::ZERO,Vec2::new(WIN_W,WIN_H)),&env);});
            });
            raster.update_textures(&output.textures_delta);
            raster.buf.fill([0.07,0.09,0.13,1.0]);
            raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
            if frame==179{raster.to_rgba8(&mut rgba);}
            raster.free_textures(&output.textures_delta);
        }
        rgba
    }
    for page in std::iter::once(None).chain((0..8).map(Some)) {
        for notice in [false,true] {
            assert_eq!(draw(1280.0,page,notice),draw(960.0,page,notice),"缩减透明余量不能改变页面 {page:?} 和通知 {notice} 的像素");
        }
    }
}


#[test]
fn compact_font_atlas_preserves_panel_and_notice_pixels(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    fn draw(texture_side:usize,page:Option<usize>,notice:bool)->Vec<u8>{
        let mut h=H::new();
        if let Some(page)=page{h.app.show_page(page);}
        let mut raster=crate::preview_raster::Raster::new(1280,270);
        let mut rgba=Vec::new();
        for frame in 0..180 {
            if notice && frame==60{h.app.alert("收到新邮件","会议通知及验证码 834921",5,true);}
            let click=(notice && h.app.compact_notices() && (frame==90 || frame==91)).then(||h.notice_button().center());
            let t=frame as f64/60.0;
            let env=Env{t,clock:"09:41:00".into(),today:"2026-10-28".into(),date:"10月28日 周三".into(),cursor:click.or_else(||page.map(|_|H::local(300.0,40.0))),..Default::default()};
            let output=h.ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(960.0,WIN_H))),max_texture_side:Some(texture_side),time:Some(t),events:click.map(|pos|vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:frame==90,modifiers:Default::default()}]).unwrap_or_default(),..Default::default()},|ctx|{
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{h.app.frame(ui,Rect::from_min_size(Pos2::ZERO,Vec2::new(WIN_W,WIN_H)),&env);});
            });
            raster.update_textures(&output.textures_delta);
            if [30,65,95,179].contains(&frame){
                raster.buf.fill([0.0;4]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
                let mut pixels=Vec::new();raster.to_rgba8(&mut pixels);
                rgba.extend(pixels);
            }
            raster.free_textures(&output.textures_delta);
        }
        rgba
    }
    for page in std::iter::once(None).chain((0..8).map(Some)) {
        for notice in [false,true] {
            let reference=draw(16384,page,notice);
            for limit in [4096,2048] {let other=draw(limit,page,notice);
                let max=reference.iter().zip(&other).map(|(a,b)|a.abs_diff(*b)).max().unwrap_or(0);let count=reference.iter().zip(&other).filter(|(a,b)|a!=b).count();
                // 图集坐标改变只允许少量通道有 1/255 的浮点采样舍入差异（实测最多约 0.005% 的通道），肉眼不可见。
                assert!(max<=1 && count<reference.len()/1000,"图集上限 {limit} 改变了页面 {page:?}、通知 {notice} 的像素：{max}/{count}");}
        }
    }
}
#[test]
fn compact_font_atlas_preserves_settings_pixels_at_multiple_scales(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    fn draw(limit:usize,scale:f32)->Vec<Vec<u8>>{
        let mut h=H::new();h.app.open_settings();h.ctx.set_embed_viewports(false);
        let output=std::sync::Arc::new(std::sync::Mutex::new(None));let capture=output.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            let raw=egui::RawInput{viewport_id:id,max_texture_side:Some(limit),screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,560.0))),
                viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(scale),..Default::default()})).collect(),..Default::default()};
            *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        let mut raster=crate::preview_raster::Raster::new((690.0*scale)as usize,(560.0*scale)as usize);let mut result=Vec::new();
        for tab in 0..12 {h.app.settings_tab=tab;
            for frame in 0..3 {
                let root=h.ctx.run(egui::RawInput{max_texture_side:Some(limit),viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(scale),..Default::default()})).collect(),..Default::default()},|ctx|h.app.settings_ui(ctx,false));
                let mut child=output.lock().unwrap().take().unwrap();
                raster.update_textures(&root.textures_delta);raster.update_textures(&child.textures_delta);
                if frame==2 {
                    // 常驻预览读取真实时钟，排除跨分钟变化；固定时钟已由主界面对照覆盖。
                    if tab==10{child.shapes.retain(|s|!matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().len()==5 && t.galley.text().as_bytes()[2]==b':'));}
                    raster.buf.fill([0.0,0.0,0.0,1.0]);raster.draw(&h.ctx.tessellate(child.shapes,child.pixels_per_point),child.pixels_per_point);
                    let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);result.push(rgba);
                }
                raster.free_textures(&root.textures_delta);raster.free_textures(&child.textures_delta);
            }
        }result
    }
    for scale in [1.0,1.5,2.0] {
        let before=draw(16384,scale);
        for limit in [4096,2048] {let after=draw(limit,scale);
        for (tab,(before,after)) in before.iter().zip(&after).enumerate(){
            // 图集坐标改变只允许少量通道有 1/255 的浮点采样舍入差异。
            let max=before.iter().zip(after).map(|(a,b)|a.abs_diff(*b)).max().unwrap_or(0);
            let count=before.iter().zip(after).filter(|(a,b)|a!=b).count();assert!(max<=1 && count<before.len()/1000,"设置页 {tab} 在图集上限 {limit}、缩放 {scale} 下出现可见差异：{max}/{count}");
        }}
    }
}












/// 新功能画面：按需导出，检查排版。
fn feature_raster()->crate::preview_raster::Raster{crate::preview_raster::Raster::new((crate::app::CANVAS_W*2.0) as usize,(WIN_H*2.0) as usize)}
fn shoot_feature(h:&mut H,raster:&mut crate::preview_raster::Raster,dest:&std::path::Path,name:&str,frames:usize,cursor:Option<Pos2>,today:&str){
    let ppp=2.0;let (w,hgt)=(crate::app::CANVAS_W,WIN_H);
    for f in 0..frames {
        h.t+=1.0/60.0;
        let env=Env{t:h.t,clock:"09:41:00".into(),today:today.into(),date:"11月4日 周三".into(),cursor,..Default::default()};
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,
            events:cursor.map(|p|vec![egui::Event::PointerMoved(p)]).unwrap_or_default(),
            viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(ppp),..Default::default()})).collect(),..Default::default()};
        let app=&mut h.app;
        let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{let win=Rect::from_min_size(ui.max_rect().min,Vec2::new(WIN_W,WIN_H));app.frame(ui,win,&env);});});
        raster.update_textures(&output.textures_delta);
        let primitives=h.ctx.tessellate(output.shapes,output.pixels_per_point);
        if f+1==frames {
            raster.buf.fill([0.20,0.24,0.32,1.0]);
            raster.draw(&primitives,output.pixels_per_point);
            let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
            let image=image::RgbaImage::from_raw((w*ppp) as u32,(hgt*ppp) as u32,rgba).unwrap();
            let crop_h=(h.app.expanded_height()*ppp+24.0).max(330.0).min(hgt*ppp) as u32;
            image::imageops::crop_imm(&image,200,0,880,crop_h).to_image().save(dest.join(format!("{name}.png"))).unwrap();
        }
        raster.free_textures(&output.textures_delta);
    }
}

#[test]
#[ignore="按需导出每周待办与剪贴板的实际画面"]
fn feature_gallery(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_FEATURE_PREVIEW_DIR").expect("需指定预览目录"));std::fs::create_dir_all(&dest).unwrap();
    let today="2026-11-04";// 周三
    let todos=||{
        let mut a=crate::model::TodoItem::new("写周报");a.repeat=0b001_0000;
        let mut b=crate::model::TodoItem::new("站会");b.repeat=crate::model::REPEAT_WORKDAYS;b.due=Some("2026-11-04 09:30".into());b.done_on=Some("2026-11-04".into());
        let mut c=crate::model::TodoItem::new("健身");c.repeat=0b000_0101;
        let d=crate::model::TodoItem::new("回复设计评审意见");
        vec![a,b,c,d]
    };
    let mut h=H::new();let mut r=feature_raster();h.app.todos.items=todos();h.app.show_page(4);
    shoot_feature(&mut h,&mut r,&dest,"todo-page",90,Some(H::local(120.0,60.0)),today);
    let mut h=H::new();let mut r=feature_raster();h.app.todos.items=todos();h.app.show_page(4);
    shoot_feature(&mut h,&mut r,&dest,"todo-editor-0",30,Some(H::local(120.0,60.0)),today);
    h.app.open_task_dialog(2);
    shoot_feature(&mut h,&mut r,&dest,"todo-editor",60,Some(H::local(120.0,60.0)),today);
    let mut h=H::new();let mut r=feature_raster();h.app.todos.items=todos();h.app.show_page(4);
    shoot_feature(&mut h,&mut r,&dest,"todo-picker-0",30,Some(H::local(120.0,60.0)),today);
    h.app.open_task_reminder(3);
    shoot_feature(&mut h,&mut r,&dest,"todo-picker",60,Some(H::local(120.0,60.0)),today);
    // 剪贴板
    let mut h=H::new();let mut r=feature_raster();h.app.show_page(3);h.app.shelf_tab=1;
    shoot_feature(&mut h,&mut r,&dest,"clip-empty",60,Some(H::local(120.0,60.0)),today);
    {let mut hist=h.app.clipboard.lock();hist.items.clear();
        for (i,t) in ["https://github.com/Renly1994/xiaotang-lingqi/releases","会议纪要：\n1. 周三前确认灵栖竖版方案\n2. 剪贴板记录上线","13800138000","SELECT * FROM todo WHERE repeat > 0 ORDER BY due;","验证码 482913"].iter().enumerate(){hist.push(t,&format!("09:{:02}",30+i));}
        hist.items[3].pinned=true;}
    shoot_feature(&mut h,&mut r,&dest,"clip-list",30,Some(H::local(200.0,66.0)),today);
    h.app.shelf_tab=0;
    let file=std::env::temp_dir().join("灵栖周报.docx");std::fs::write(&file,b"x").unwrap();h.app.shelf.items=vec![file];
    shoot_feature(&mut h,&mut r,&dest,"shelf-files",30,Some(H::local(120.0,60.0)),today);
    h.app.confirm_clear=Some(h.app.t);
    shoot_feature(&mut h,&mut r,&dest,"shelf-confirm",5,Some(H::local(120.0,60.0)),today);
    h.app.settings.clipboard_history=false;h.app.shelf_tab=1;
    shoot_feature(&mut h,&mut r,&dest,"clip-off",30,Some(H::local(120.0,60.0)),today);
}

static SIDE_MUSIC:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
/// 侧边停靠的画面：画布为 SIDE_W × SIDE_H。
fn shoot_side(h:&mut H,raster:&mut crate::preview_raster::Raster,dest:&std::path::Path,name:&str,frames:usize,cursor:Option<Pos2>){
    shoot_side_with_bounds(h,raster,dest,name,frames,cursor,true);
}
fn shoot_side_with_bounds(h:&mut H,raster:&mut crate::preview_raster::Raster,dest:&std::path::Path,name:&str,frames:usize,cursor:Option<Pos2>,draw_bounds:bool){
    let ppp=2.0;let (w,hgt)=(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H);
    for f in 0..frames {
        h.t+=1.0/60.0;
        let mut env=Env{t:h.t,clock:"09:41:00".into(),today:"2026-11-04".into(),date:"11月4日 周三".into(),cursor,battery:Some(76),..Default::default()};
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,
            events:cursor.map(|p|vec![egui::Event::PointerMoved(p)]).unwrap_or_default(),
            viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(ppp),..Default::default()})).collect(),..Default::default()};
        let app=&mut h.app;let mut out=None;
        let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{let win=Rect::from_min_size(ui.max_rect().min,Vec2::new(w,hgt));out=Some(app.frame(ui,win,&env));});});
        raster.update_textures(&output.textures_delta);
        if f+1==frames {
            raster.buf.fill([0.20,0.24,0.32,1.0]);
            raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
            let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
            let mut image=image::RgbaImage::from_raw((w*ppp) as u32,(hgt*ppp) as u32,rgba).unwrap();
            // 画出窗口区域（红）与命中区域（绿）的边框，方便检查。
            if let Some(out)=out.as_ref().filter(|_|draw_bounds) {
                for (r,color) in [(out.region,image::Rgba([255,80,80,255])),(out.hit,image::Rgba([80,255,120,255]))] {
                    let (x0,y0,x1,y1)=((r.left()*ppp).max(0.0) as u32,(r.top()*ppp).max(0.0) as u32,((r.right()*ppp) as u32).min(image.width()-1),((r.bottom()*ppp) as u32).min(image.height()-1));
                    for x in x0..=x1 {image.put_pixel(x,y0,color);image.put_pixel(x,y1,color);}
                    for y in y0..=y1 {image.put_pixel(x0,y,color);image.put_pixel(x1,y,color);}
                }
            }
            if !draw_bounds{
                if let Some(out)=out.as_ref().filter(|out|out.region.is_positive()){
                    let rect=out.region.expand(8.0).intersect(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt)));
                    image=image::imageops::crop_imm(&image,(rect.left()*ppp)as u32,(rect.top()*ppp)as u32,
                        (rect.width()*ppp)as u32,(rect.height()*ppp)as u32).to_image();
                }
            }
            image.save(dest.join(format!("{name}.png"))).unwrap();
        }
        raster.free_textures(&output.textures_delta);
    }
}

fn feature_raster_sized(w:f32,h:f32)->crate::preview_raster::Raster{crate::preview_raster::Raster::new((w*2.0) as usize,(h*2.0) as usize)}

#[test]
fn side_dock_auto_minimal_follows_avoidance(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let (w,hgt)=(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H);
    let mut h=H::new();h.app.settings.dock=crate::model::Dock::Right;h.app.settings.browser_minimal=true;
    let away=Some(Pos2::new(w/2.0,hgt-5.0));
    for _ in 0..60 {side_frame(&mut h,away);}
    let normal=side_frame(&mut h,away).hit;
    h.browser_at_top=Some(true);
    let mut out=side_frame_with(&mut h,away);
    for _ in 0..60 {out=side_frame_with(&mut h,away);}
    assert!(out.hit.width()<normal.width() && out.hit.height()<normal.height(),"避让时收成贴边细线：{:?} → {:?}",normal,out.hit);
    h.browser_at_top=Some(false);
    for _ in 0..60 {out=side_frame_with(&mut h,away);}
    assert!((out.hit.height()-normal.height()).abs()<1.0,"离开后恢复：{:?}",out.hit);
}

fn side_frame_with(h:&mut H,cursor:Option<Pos2>)->crate::app::FrameOut{
    let at_top=h.browser_at_top;
    let (w,hgt)=(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H);
    h.t+=1.0/60.0;
    let env=Env{t:h.t,clock:"09:41:00".into(),cursor,battery:Some(76),browser_at_top:at_top,..Default::default()};
    let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,..Default::default()};
    let app=&mut h.app;let mut out=None;
    let _=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{out=Some(app.frame(ui,Rect::from_min_size(ui.max_rect().min,Vec2::new(w,hgt)),&env));});});
    out.unwrap()
}

fn side_frame(h:&mut H,cursor:Option<Pos2>)->crate::app::FrameOut{
    side_frame_events(h,cursor,cursor.map(|p|vec![egui::Event::PointerMoved(p)]).unwrap_or_default())
}

fn side_frame_events(h:&mut H,cursor:Option<Pos2>,events:Vec<egui::Event>)->crate::app::FrameOut{
    let (w,hgt)=(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H);
    h.t+=1.0/60.0;
    let env=Env{t:h.t,clock:"09:41:00".into(),today:"2026-11-04".into(),date:"11月4日 周三".into(),cursor,battery:Some(76),..Default::default()};
    let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,
        events,..Default::default()};
    let app=&mut h.app;let mut out=None;
    let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{out=Some(app.frame(ui,Rect::from_min_size(ui.max_rect().min,Vec2::new(w,hgt)),&env));});});
    h.shapes=output.shapes;
    out.unwrap()
}

#[test]
fn side_dock_opens_from_the_edge_and_collapses_back(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let (w,hgt)=(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H);
    for (dock,shape) in [(crate::model::Dock::Left,crate::model::IslandShape::Notch),(crate::model::Dock::Right,crate::model::IslandShape::Pill)] {
        let mut h=H::new();h.app.settings.dock=dock;h.app.settings.island_shape=shape;
        let away=Some(Pos2::new(w/2.0,hgt-5.0));
        let mut out=side_frame(&mut h,away);
        for _ in 0..60 {out=side_frame(&mut h,away);}
        assert!(!h.app.open);
        // 收起：竖向胶囊贴边、竖直居中。
        assert!(out.hit.height()>out.hit.width(),"{dock:?} 收起时是竖向：{:?}",out.hit);
        assert!((out.hit.center().y-hgt/2.0).abs()<1.0);
        let edge=if dock==crate::model::Dock::Left{0.5}else{w-0.5};
        assert!(out.hit.contains(Pos2::new(edge,hgt/2.0)),"{dock:?} 光标推到屏幕边缘也能命中：{:?}",out.hit);
        assert!(out.region.contains_rect(out.hit.shrink(0.5)));
        // 推到边缘停留 → 展开，面板从边缘滑出且完整留在画布内。
        for _ in 0..60 {out=side_frame(&mut h,Some(Pos2::new(edge,hgt/2.0)));}
        assert!(h.app.open && h.app.progress()>0.95,"{dock:?} 应展开");
        assert!(out.hit.width()>crate::app::PW-1.0 && out.hit.height()<200.0,"{:?}",out.hit);
        assert!(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt)).contains_rect(out.region),"区域不能超出画布：{:?}",out.region);
        if dock==crate::model::Dock::Left {assert!(out.hit.left()<=0.5)} else {assert!(out.hit.right()>=w-0.5)}
        // 离开 → 收起。
        for _ in 0..90 {out=side_frame(&mut h,away);}
        assert!(!h.app.open && h.app.progress()<0.05);
        assert!(out.hit.width()<60.0);
    }
}

#[test]
fn side_dock_file_drag_opens_the_shelf_and_drop_saves_it(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let (w,hgt)=(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H);
    let path=crate::model::config_dir().join("侧边拖入.txt");std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(&path,"x").unwrap();
    for dock in [crate::model::Dock::Left,crate::model::Dock::Right] {
        let mut h=H::new();h.app.settings.dock=dock;
        let edge=if dock==crate::model::Dock::Left{10.0}else{w-10.0};
        let at=Pos2::new(edge,hgt/2.0);
        let mut frame=|h:&mut H,hovered:bool,dropped:bool|{
            h.t+=1.0/60.0;
            let env=Env{t:h.t,clock:"09:41:00".into(),cursor:Some(at),..Default::default()};
            let mut raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,..Default::default()};
            if hovered {raw.hovered_files.push(egui::HoveredFile{path:Some(path.clone()),..Default::default()});}
            if dropped {raw.dropped_files.push(egui::DroppedFile{path:Some(path.clone()),..Default::default()});}
            let app=&mut h.app;let mut out=None;
            let _=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{out=Some(app.frame(ui,Rect::from_min_size(ui.max_rect().min,Vec2::new(w,hgt)),&env));});});
            out.unwrap()
        };
        let out=frame(&mut h,false,false);
        let zone=out.file_drop_zone.expect("置物架开启时侧边也有拖放感应区");
        assert!(zone.contains(at),"{dock:?} 感应区应覆盖贴边的胶囊：{zone:?}");
        frame(&mut h,true,false);
        assert!(h.app.drag_hot && h.app.page==3,"{dock:?} 文件拖到胶囊上应切到置物架");
        for _ in 0..40 {frame(&mut h,true,false);}
        assert!(h.app.open);
        frame(&mut h,false,true);
        assert!(h.app.shelf.items.contains(&path),"{dock:?} 松手后应放进置物架");
        h.app.shelf.items.clear();h.app.shelf.save();
    }
}

#[test]
fn shelf_menu_last_action_fits_inside_window_and_works(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.shelf.items=vec![r"folder\menu-test.zip".into()];
    h.app.show_page(3);
    let point=H::local(209.5,101.0);
    h.hover(point,1.2);
    for pressed in [true,false]{
        h.frame(vec![egui::Event::PointerButton{pos:point,button:egui::PointerButton::Secondary,pressed,modifiers:Default::default()}]);
    }
    h.hover(point,1.5);
    assert!(!h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==r"folder\menu-test.zip")));
    let label=h.shapes.iter().find_map(|s|match &s.shape{
        egui::Shape::Text(t) if t.galley.text()=="从置物架移除"=>Some(Rect::from_min_size(t.pos,t.galley.size())),
        _=>None,
    }).expect("右键菜单应显示移除操作");
    assert!(h.region.contains_rect(h.app.shelf_menu_rect.unwrap() + h.ctx.style().visuals.popup_shadow.margin()));
    assert!(h.app.shelf_menu_rect.unwrap().contains_rect(label));
    assert!(h.top_bar_notice.is_none(),"置物架菜单不能触发通知置顶");
    h.hover(label.center(),1.0);
    assert!(h.app.open);
    h.click(label.center());
    assert!(h.app.shelf.items.is_empty());
}










#[test]
fn weather_page_shows_the_local_snapshot_and_forecast(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings.pet=false;
    h.app.weather.snapshot=Some(crate::weather::Snapshot{condition:None,attributions:vec![],place:"杭州".into(),position:None,auto:false,temp:9.0,feels:Some(5.6),humidity:Some(58),wind:Some(13.9),code:51,day:true,high:Some(15.2),low:Some(6.8),
        sunrise:Some("06:52".into()),sunset:Some("17:21".into()),sun_times:None,
        days:vec![
            crate::weather::Day{label:"今天".into(),date:"2026-11-09".into(),high:15.2,low:6.8,code:51,rain:60},
            crate::weather::Day{label:"明天".into(),date:"2026-11-10".into(),high:14.5,low:4.7,code:2,rain:0},
            crate::weather::Day{label:"周三".into(),date:"2026-11-11".into(),high:17.4,low:5.0,code:2,rain:0},
            crate::weather::Day{label:"周四".into(),date:"2026-11-12".into(),high:19.2,low:8.2,code:61,rain:25},
        ],at:chrono::Utc::now().timestamp()});
    h.app.show_page(8);h.hover(H::local(300.0,60.0),1.0);
    assert_eq!(h.app.page,8,"天气应是第九个页面");
    for phrase in ["9°","毛毛雨","杭州","今天","明天","周三","周四","60%","体感","6°","湿度","58%","日出 06:52","日落 17:21"]{
        assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains(phrase))),"天气页缺少：{phrase}");
    }
    for s in &h.shapes{if let egui::Shape::Text(t)=&s.shape{assert!(s.clip_rect.contains_rect(t.visual_bounding_rect()),"天气页文字被裁切：{}",t.galley.text());}}
    if let Some(dest)=std::env::var_os("LINGQI_WEATHER_PREVIEW_DIR"){
        let dest=std::path::PathBuf::from(dest);std::fs::create_dir_all(&dest).unwrap();
        let mut raster=feature_raster();shoot_feature(&mut h,&mut raster,&dest,"weather",30,Some(H::local(300.0,60.0)),"2026-11-09");
    }
}

#[test]
fn weather_page_explains_when_data_is_missing(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.weather.error=Some("网络不可用".into());
    h.app.show_page(8);h.hover(H::local(300.0,60.0),1.0);
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()=="天气暂时取不到")));
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("网络不可用"))));
}

#[test]
fn weather_chip_sits_after_the_clock_on_the_resident_bar(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();h.app.settings.pet=false;
    h.app.weather.snapshot=Some(crate::weather::Snapshot{condition:None,attributions:vec![],place:"杭州".into(),position:None,auto:false,temp:9.0,feels:Some(5.6),humidity:Some(58),wind:Some(13.9),code:51,day:true,high:Some(15.2),low:Some(6.8),sunrise:Some("06:52".into()),sunset:Some("17:21".into()),sun_times:None,days:vec![],at:chrono::Utc::now().timestamp()});
    for _ in 0..60{h.frame(vec![]);}
    let layout=h.app.resident.current();
    let clock=layout.x[crate::bar_layout::CLOCK].expect("时钟应常驻");
    let chip=layout.x[crate::bar_layout::WEATHER].expect("有快照时天气徽标应紧跟时钟占位");
    assert!(chip>clock+22.0+6.9,"天气应在时钟右侧：时钟 {clock}，天气 {chip}");
    let clock_text=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()=="09:41"=>Some(t.visual_bounding_rect()),_=>None}).expect("常驻栏应有时钟");
    let temp=h.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()=="9°"=>Some(t.visual_bounding_rect()),_=>None}).expect("常驻栏应在时间后显示当前温度");
    assert!(temp.left()>clock_text.right()+4.0,"温度应与时钟留出间距：时钟右边 {:?}，温度左边 {:?}",clock_text.right(),temp.left());
    assert!(temp.center().y<30.0,"温度应与图标同一行");
    h.click(temp.center());
    assert_eq!(h.app.page(),8,"点温度也能打开天气页");
}

#[test]
fn weather_chip_hides_without_data_or_when_the_page_is_hidden(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();h.app.settings.pet=false;
    for _ in 0..60{h.frame(vec![]);}
    assert!(h.app.resident.current().x[crate::bar_layout::WEATHER].is_none(),"没有快照时不占位");
    h.app.weather.snapshot=Some(crate::weather::Snapshot{condition:None,attributions:vec![],place:"杭州".into(),position:None,auto:false,temp:9.0,feels:None,humidity:None,wind:None,code:0,day:true,high:None,low:None,sunrise:None,sunset:None,sun_times:None,days:vec![],at:chrono::Utc::now().timestamp()});
    for _ in 0..60{h.frame(vec![]);}
    assert!(h.app.resident.current().x[crate::bar_layout::WEATHER].is_some(),"有快照时占位");
    h.app.settings.visible_pages[8]=false;
    for _ in 0..60{h.frame(vec![]);}
    assert!(h.app.resident.current().x[crate::bar_layout::WEATHER].is_none(),"天气页被隐藏后不占位");
    assert!(!h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text()=="9°")),"隐藏后不再画温度");
}


#[test]
fn weather_icons_stay_bright_on_light_skins(){
    use crate::skin_paint::{map_color,CREAM,GLASS_LIGHT,GLASS_DARK};
    let luma=|c:egui::Color32|0.299*c.r() as f32+0.587*c.g() as f32+0.114*c.b() as f32;
    let cloud=egui::Color32::from_rgb(238,242,250);let sun=egui::Color32::from_rgb(255,196,72);
    for palette in [CREAM,GLASS_LIGHT]{
        // 通用换色会把白云反成深色、把太阳压成暗金；天气图标改用自己的浅色版配色。
        assert!(luma(map_color(cloud,&palette))<90.0);
        assert!(luma(crate::weather_ui::light_tone(cloud,&palette))>190.0,"浅色皮肤下的云应是淡蓝灰");
        let bright=crate::weather_ui::light_tone(sun,&palette);
        assert!(bright.r()>=250&&bright.g()>=160&&bright.b()<80,"太阳保持明亮暖黄：{bright:?}");
        let half=crate::weather_ui::light_tone(sun.gamma_multiply(0.5),&palette);assert_eq!(half.a(),sun.gamma_multiply(0.5).a(),"保留透明度");
    }
    assert_eq!(crate::weather_ui::light_tone(cloud,&GLASS_DARK),map_color(cloud,&GLASS_DARK),"深色皮肤不受影响");
}

#[test]
#[ignore="按需导出常驻天气的全部图标样式"]
fn weather_icon_atlas_preview(){
    use crate::{paint::{Card,H as Align},theme::{self,W},weather::{self,Snapshot}};
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_WEATHER_PREVIEW_DIR").expect("需指定预览目录"));std::fs::create_dir_all(&dest).unwrap();
    let cases=[("晴 · 白天",0,true,"0"),("晴 · 夜间",0,false,"0"),("晴间多云 · 白天",2,true,"1 / 2"),
        ("晴间多云 · 夜间",2,false,"1 / 2"),("阴天",3,true,"3"),("雾 / 雾凇",45,true,"45 / 48"),
        ("细雨 / 冻毛毛雨",51,true,"51 / 53 / 55 / 56 / 57"),("雨 / 冻雨",63,true,"61 / 63 / 65 / 66 / 67"),("阵雨",80,true,"80 / 81 / 82"),
        ("雪 / 阵雪",73,true,"71 / 73 / 75 / 77 / 85 / 86"),("雷雨 / 冰雹",95,true,"95 / 96 / 99"),("未知代码 · 回退云图标",200,true,"其他代码")];
    for (name,palette,ppp) in [("weather-icons-dark",None,2.0f32),("weather-icons-cream",Some(crate::skin_paint::CREAM),2.0),("weather-icons-glass-light",Some(crate::skin_paint::GLASS_LIGHT),2.0),("weather-icons-dark-1x",None,1.0),("weather-icons-dark-125",None,1.25),("weather-icons-dark-150",None,1.5)] {
        let (pw,ph)=((924.0*ppp) as usize,(700.0*ppp) as usize);
        let mut h=H::new();h.app.settings.pet=false;
        let mut layout=crate::bar_layout::Layout::new(false,false,false,false,false,true);layout.widths[crate::bar_layout::WEATHER]=50.0;
        let mut raster=crate::preview_raster::Raster::new(pw,ph);
        for frame in 0..4 {
            let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(924.0,700.0))),time:Some(frame as f64),
                viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(ppp),..Default::default()})).collect(),..Default::default()};
            let mut output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{
                let c=Card::new(ui,Pos2::ZERO,ui.max_rect());c.fill(0.0,0.0,924.0,700.0,0.0,egui::Color32::from_rgb(14,15,19));
                c.text(20.0,38.0,"常驻天气图标",22.0,W::Bold,theme::WHITE,Align::Left);
                c.text(20.0,61.0,"11类图标 · 顶部横排与侧边竖排 · 当前实际绘制样式",11.5,W::Regular,theme::GRAY,Align::Left);
                for (index,&(label,code,day,codes)) in cases.iter().enumerate(){
                    let tile=c.shifted(20.0+(index%3)as f32*298.0,84.0+(index/3)as f32*150.0);
                    tile.fill(0.0,0.0,286.0,138.0,11.0,egui::Color32::from_rgb(25,25,29));
                    tile.text(14.0,23.0,label,12.5,W::Semibold,theme::WHITE,Align::Left);
                    crate::weather_ui::icon(&tile,32.0,63.0,33.0,weather::kind(code,day));
                    let snapshot=Snapshot{place:"杭州".into(),temp:if code==73{-6.0}else{19.0},code,day,at:chrono::Utc::now().timestamp(),..Default::default()};
                    tile.fill(65.0,46.0,148.0,30.0,15.0,egui::Color32::BLACK);
                    let bar=tile.shifted(65.0,46.0);bar.text(32.0,21.0,"10:18",15.5,W::Bold,theme::WHITE,Align::Center);
                    h.app.paint_pill_weather(&bar,105.0,&layout,&snapshot);
                    tile.fill(234.0,40.0,36.0,47.0,18.0,egui::Color32::BLACK);
                    h.app.paint_side_weather(&tile.shifted(234.0,40.0),18.0,23.0,&snapshot);
                    for (x,label) in [(32.0,"放大"),(139.0,"顶部 15px"),(252.0,"侧边")]{tile.text(x,104.0,label,9.5,W::Regular,theme::GRAY,Align::Center);}
                    tile.text(14.0,125.0,&format!("WMO  {codes}"),8.5,W::Regular,theme::DIM,Align::Left);
                }
                c.text(20.0,690.0,"无数据、关闭天气或极简模式时不显示；超过半小时未更新时，温度转灰。",10.5,W::Regular,theme::GRAY,Align::Left);
                // 和岛上一样按图层换色，天气图标登记的浅色配色才会生效。
                if let Some(palette)=palette {let layer=ui.layer_id();crate::skin_paint::recolor_layer(ctx,layer,&[(0,crate::skin_paint::shape_count(ctx,layer))],&palette);}
            });});
            raster.update_textures(&output.textures_delta);
            if frame==3 {raster.buf.fill([0.0,0.0,0.0,1.0]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);image::save_buffer(dest.join(format!("{name}.png")),&rgba,pw as u32,ph as u32,image::ColorType::Rgba8).unwrap();}
            raster.free_textures(&output.textures_delta);
        }
    }
}

#[test]
#[ignore="按需导出天气设置页，检查排版"]
fn weather_settings_preview(){
    use crate::settings_theme::SettingsTheme;
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_WEATHER_SETTINGS_PREVIEW").expect("需指定预览目录"));std::fs::create_dir_all(&dest).unwrap();
    for (tag,theme,dropdown) in [("dark",SettingsTheme::Dark,false),("light",SettingsTheme::Light,false),("dropdown-dark",SettingsTheme::Dark,true),("dropdown-light",SettingsTheme::Light,true)] {
        let mut h=H::new();h.app.settings.weather_city="杭州".into();h.app.weather.city_input="杭州".into();h.app.weather.manual_city=true;h.app.settings.settings_theme=theme;
        h.ctx.style_mut(|style|style.animation_time=0.0);
        let current=crate::weather::Position{lat:30.27,lon:120.15,label:"杭州".into(),region:"浙江 · 中国".into()};
        h.app.weather.selected_city=Some(current.clone());h.app.weather.active_position=Some(current.clone());
        h.app.settings.weather_position=Some(current.clone());h.app.settings.weather_saved=vec![current.clone(),crate::weather::Position{lat:31.23,lon:121.47,label:"上海".into(),region:"上海 · 中国".into()}];
        h.app.weather.snapshot=Some(crate::weather::Snapshot{place:"杭州".into(),temp:19.0,code:2,day:true,feels:Some(18.0),humidity:Some(58),wind:Some(12.0),high:Some(23.0),low:Some(15.0),
            sunrise:Some("06:52".into()),sunset:Some("17:21".into()),at:chrono::Utc::now().timestamp(),..Default::default()});
        h.app.weather.snapshot.as_mut().unwrap().position=Some(current);
        if dropdown {
            h.app.weather.city_input="佛山".into();
            h.app.weather.cities.seed("佛山",vec![
                crate::weather::Position{lat:23.02677,lon:113.13148,label:"佛山市".into(),region:"广东 · 中国".into()},
                crate::weather::Position{lat:28.73401,lon:98.80612,label:"佛山".into(),region:"迪庆藏族自治州 · 云南 · 中国".into()},
                crate::weather::Position{lat:30.04572,lon:106.41138,label:"佛山".into(),region:"重庆市 · 中国".into()},
            ]);
        }
        h.app.settings_tab=12;h.app.open_settings();h.ctx.set_embed_viewports(false);
        let children=std::sync::Arc::new(Mutex::new(Vec::new()));let capture=children.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(800.0,620.0))),
                viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(2.0),..Default::default()})).collect(),..Default::default()};
            capture.lock().unwrap().push(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        let mut raster=crate::preview_raster::Raster::new(1600,1240);
        for frame in 0..4 {
            if dropdown && frame==1 {h.ctx.memory_mut(|m|m.open_popup(egui::Id::new("weather-city-options")));}
            let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(crate::app::CANVAS_W,WIN_H))),time:Some(frame as f64),..Default::default()};
            let root=h.ctx.run(raw,|ctx|h.app.settings_ui(ctx,false));
            let child_frames:Vec<_>=children.lock().unwrap().drain(..).collect();
            for child in &child_frames {raster.update_textures(&child.textures_delta);}
            raster.update_textures(&root.textures_delta);
            for child in child_frames {
                if frame==3 {raster.buf.fill([0.1,0.1,0.1,1.0]);raster.draw(&h.ctx.tessellate(child.shapes,child.pixels_per_point),child.pixels_per_point);let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);image::save_buffer(dest.join(format!("weather-settings-{tag}.png")),&rgba,1600,1240,image::ColorType::Rgba8).unwrap();}
                raster.free_textures(&child.textures_delta);
            }
            raster.free_textures(&root.textures_delta);
        }
        assert!(dest.join(format!("weather-settings-{tag}.png")).exists(),"必须导出真实设置窗口");
    }
}

#[test]
#[cfg(windows)]
fn settings_window_memory_restores_only_the_child_viewport_and_keeps_live_resizing(){
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();h.ctx.set_embed_viewports(false);h.app.open_settings();
    let original=(h.app.settings.island_size,h.app.settings.dock,h.app.settings.display_id.clone());
    let size=std::sync::Arc::new(std::sync::Mutex::new(egui::vec2(900.0,600.0)));
    let resized=size.clone();
    egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
        let id=viewport.ids.this;let size=*resized.lock().unwrap();
        let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,size)),
            viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),
                inner_rect:Some(Rect::from_min_size(egui::pos2(100.0,140.0),size)),
                outer_rect:Some(Rect::from_min_size(egui::pos2(92.0,100.0),size+egui::vec2(16.0,48.0))),..Default::default()})).collect(),..Default::default()};
        let _=ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx));
    });
    let frame=|h:&mut H|h.ctx.run(egui::RawInput{viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(1.75),..Default::default()})).collect(),..Default::default()},|ctx|h.app.settings_ui(ctx,false));
    let _=frame(&mut h);h.app.settings_open=false;let _=frame(&mut h);
    *size.lock().unwrap()=egui::vec2(690.0,560.0);h.app.open_settings();let root=frame(&mut h);
    assert!(root.viewport_output.iter().filter(|(id,_)|**id!=egui::ViewportId::ROOT).flat_map(|(_,v)|&v.commands).any(|c|matches!(c,egui::ViewportCommand::OuterPosition(_))),"重开时应恢复子窗口");
    assert!(!root.viewport_output.get(&egui::ViewportId::ROOT).unwrap().commands.iter().any(|c|matches!(c,egui::ViewportCommand::OuterPosition(_)|egui::ViewportCommand::InnerSize(_))));
    *size.lock().unwrap()=egui::vec2(1100.0,700.0);let live=frame(&mut h);
    assert!(!live.viewport_output.values().flat_map(|v|&v.commands).any(|c|matches!(c,egui::ViewportCommand::InnerSize(_))),"正常拖动不能每帧强制恢复旧尺寸");
    assert_eq!((h.app.settings.island_size,h.app.settings.dock,h.app.settings.display_id.clone()),original);
}



















fn raised(h:&H,title:&str)->bool{
    h.app.alerts.iter().chain(h.app.notice.as_ref()).any(|alert|alert.title==title)
}

#[test]
fn closed_modules_stop_their_reminders_and_timers(){
    use crate::app::{FOCUS_PAGE,TODO_PAGE,WATER_PAGE};
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    for open in [true,false] {
        let mut h=H::new();h.frame(vec![]);
        h.app.pomo.running=true;h.app.pomo.remaining=0.5;
        for page in [FOCUS_PAGE,TODO_PAGE,WATER_PAGE] {h.app.set_page_visible(page,open);}
        h.app.water.remind=true;h.app.water.drunk_ml=0;h.app.last_reminder=h.t-1e6;
        h.app.todos.items.push(crate::model::TodoItem{text:"交周报".into(),due:Some("2026-10-28 09:40".into()),..Default::default()});
        if !open {assert!(!h.app.pomo.running,"关闭专注模块时立即停止计时");h.app.pomo.running=true;}
        for _ in 0..60 {h.frame(vec![]);}
        assert_eq!(raised(&h,"喝水时间到了"),open,"饮水模块开启={open}");
        assert_eq!(raised(&h,"待办到时间了"),open,"待办模块开启={open}");
        assert_eq!(raised(&h,"专注计时"),open,"专注模块开启={open}");
        if !open {assert!(!h.app.pomo.running && h.app.todos.items[0].due.is_some() && !h.app.todos.items[0].reminded,"关闭期间不消耗提醒，重新开启后仍会提醒");}
    }
}


#[test]
fn performance_badge_opens_overview_and_drills_into_details() {
    use crate::{app::PERFORMANCE_PAGE,bar_layout::PERFORMANCE,performance::{BarStyle,View},performance_ui::{overview_card_center,view_tab_center,BACK}};
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.set_page_visible(PERFORMANCE_PAGE,true);
    h.app.settings.performance_bar=BarStyle::Pulse;
    for _ in 0..40 {h.frame(vec![]);}
    let badge=h.ctx.read_response(egui::Id::new(("card",true,"performance-open"))).expect("应绘制可交互的性能徽标");
    assert!(badge.rect.is_positive(),"{:?}",badge.rect);
    h.click(badge.rect.center());
    assert_eq!(h.app.debug_state(),(PERFORMANCE_PAGE,true));
    assert_eq!(h.app.performance.view,None,"展开先显示设置里的总览");
    let tap=|h:&mut H,(x,y):(f32,f32)|{h.hover(H::local(x,y),0.8);h.click(H::local(x,y));};
    tap(&mut h,overview_card_center(View::Gpu));
    assert_eq!(h.app.performance.view,Some(View::Gpu),"点总览卡片进入对应详情");
    tap(&mut h,view_tab_center(3));
    assert_eq!(h.app.performance.view,Some(View::Disk),"详情页签可以横向切换");
    tap(&mut h,(BACK[0]+BACK[2]/2.,BACK[1]+BACK[3]/2.));
    assert_eq!(h.app.performance.view,Some(View::Overview),"返回键回到总览");
    assert_eq!(h.app.settings.performance_view,View::Overview,"岛内浏览不改设置");
    h.app.performance.view=Some(View::Memory);
    h.hover(Pos2::new(5.0,WIN_H-5.0),2.0);
    assert!(!h.app.open,"移开鼠标后岛收起");
    assert_eq!(h.app.performance.view,Some(View::Memory),"收起后再展开仍停在刚才的详情");
    h.app.set_page_visible(PERFORMANCE_PAGE,false);
    h.frame(vec![]);
    assert!(h.app.resident.current().x[PERFORMANCE].is_none());
    assert_eq!(h.app.performance.view,None,"离开页面后下次从设置里的页面开始");
}

#[test]
fn wide_resident_island_stays_inside_the_window_region_while_morphing() {
    use crate::performance::{BarStyle,Metric};
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.set_page_visible(crate::app::PERFORMANCE_PAGE,true);
    h.app.settings.performance_bar=BarStyle::Custom;
    h.app.settings.performance_metrics=Metric::ALL.iter().fold(0,|bits,m|bits|m.bit());
    for _ in 0..30 {h.frame(vec![]);}
    assert!(h.app.resident.target.width>PW+20.0,"常驻条要比展开面板宽：{}",h.app.resident.target.width);
    let mut widest=0.0_f32;
    for f in 0..160 {
        h.cursor=Some(if f<70 {Pos2::new(WIN_W/2.0,15.0)} else {Pos2::new(5.0,WIN_H-5.0)});
        h.frame(vec![]);
        let island=h.app.current_width();
        widest=widest.max(island);
        assert!(h.region.width()>=island,"第 {f} 帧窗口区域 {} 窄于岛宽 {island}，两侧会被系统切掉",h.region.width());
    }
    assert!(widest>PW+20.0);
}

#[cfg(windows)]
#[test]
fn performance_program_list_scrolls_under_the_wheel_without_turning_pages() {
    use crate::{app::PERFORMANCE_PAGE,performance::{Process,View},performance_ui::list_area};
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.set_page_visible(PERFORMANCE_PAGE,true);
    h.app.show_page(PERFORMANCE_PAGE);
    h.app.performance.view=Some(View::Memory);
    h.app.performance.latest.by_memory=(0..10).map(|i|Process{name:format!("程序 {i}"),memory:(10-i) as u64*1048576,..Default::default()}).collect();
    let list=list_area().center();
    for (delta,first) in [(-40.0_f32,1),(-400.0,5),(-40.0,5),(40.0,4),(400.0,0),(40.0,0)] {
        h.hover(H::local(list.x,list.y),0.3);
        for _ in 0..(delta.abs() as usize/40) {h.wheel(delta.signum()*40.0);}
        assert_eq!(h.app.page,PERFORMANCE_PAGE,"程序列表上滚到首尾都不翻页");
        assert_eq!(h.app.performance.list_first,first);
    }
    h.app.performance.list_first=3;
    h.app.performance.view=Some(View::Cpu);
    h.frame(vec![]);
    assert_eq!(h.app.performance.list_first,0,"换到别的详情页时列表回到顶部");
    h.app.performance.view=Some(View::Memory);
    h.hover(H::local(80.0,100.0),0.3);
    h.wheel(-40.0);
    assert_ne!(h.app.page,PERFORMANCE_PAGE,"列表外的滚轮仍然翻页");
}

#[cfg(windows)]
#[test]
fn performance_keeps_sampling_across_hidden_and_open_views() {
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.set_page_visible(crate::app::PERFORMANCE_PAGE,true);
    h.app.settings.performance_bar=crate::performance::BarStyle::Network;
    h.fullscreen=true;
    h.frame(vec![]);
    std::thread::sleep(std::time::Duration::from_millis(3400));
    h.fullscreen=false;
    h.frame(vec![]);
    assert!(h.app.performance.history.len()>=3,"隐藏时也要按秒采样");
    assert!(h.app.performance.latest.cpu.is_some());
    let at=h.app.performance.history.back().unwrap().0;
    h.app.show_page(crate::app::PERFORMANCE_PAGE);
    h.frame(vec![]);
    assert!(h.app.performance.latest.cpu.is_some() && h.app.performance.history.back().unwrap().0>=at);
    h.app.set_page_visible(crate::app::PERFORMANCE_PAGE,false);
    h.frame(vec![]);
    assert!(h.app.performance.history.is_empty());
}

#[cfg(windows)]
#[test]
#[ignore="读取一分钟实际性能数据，导出常驻与展开预览"]
fn performance_gallery() {
    use crate::performance::{View,BarStyle,Metric};
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dir=std::path::PathBuf::from(std::env::var_os("LINGQI_PERFORMANCE_PREVIEW_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let mut h=H::new();
    h.app.set_page_visible(crate::app::PERFORMANCE_PAGE,true);
    let start=std::time::Instant::now();
    while start.elapsed().as_secs()<61 {
        h.app.performance.tick(true,true,true,&h.ctx);
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    let today=chrono::Local::now().date_naive().to_string();
    let mut raster=feature_raster();
    h.app.show_page(crate::app::PERFORMANCE_PAGE);
    for view in View::ALL {
        h.app.performance.view=Some(view);
        shoot_feature(&mut h,&mut raster,&dir,&format!("page-{}",view.key()),75,Some(H::local(28.,228.)),&today);
    }
    // 悬停内存点阵：程序那一段高亮。
    h.app.performance.view=Some(View::Memory);
    shoot_feature(&mut h,&mut raster,&dir,"memory-hover",20,Some(H::local(170.,110.)),&today);
    // 悬停构成条最右边的空闲段：「可用」整段（缓存 + 空闲）一起亮，标题右侧写可用总量。
    shoot_feature(&mut h,&mut raster,&dir,"memory-available-hover",20,Some(H::local(374.,110.)),&today);
    // 悬停内存页的第二个程序：构成条里属于它的那一截亮起，其余压暗。
    shoot_feature(&mut h,&mut raster,&dir,"memory-program-hover",20,Some(H::local(230.,268.)),&today);
    // 悬停总览的处理器卡片。
    h.app.performance.view=Some(View::Overview);
    shoot_feature(&mut h,&mut raster,&dir,"overview-hover",40,Some(H::local(80.,90.)),&today);
    h.app.performance.view=None;
    let all_metrics=Metric::ALL.iter().fold(0,|bits,m|bits|m.bit());
    h.app.settings.performance_metrics=all_metrics;
    for style in BarStyle::ALL {
        h.app.settings.performance_bar=style;
        shoot_feature(&mut h,&mut raster,&dir,style.key(),240,None,&today);
    }
    // 设置页：深色 / 浅色各一张，再加一张自由搭配。
    h.app.open_settings();h.ctx.set_embed_viewports(false);h.app.settings_tab=15;
    let mut settings_raster=crate::preview_raster::Raster::new(690,1240);
    let output=std::sync::Arc::new(std::sync::Mutex::new(None));let capture=output.clone();
    egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
        let id=viewport.ids.this;
        let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,1240.0))),
            viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()};
        *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
    });
    for (name,theme,style) in [("settings-dark",crate::settings_theme::SettingsTheme::Dark,BarStyle::Dashboard),
        ("settings-light",crate::settings_theme::SettingsTheme::Light,BarStyle::Gauges),
        ("settings-custom",crate::settings_theme::SettingsTheme::Dark,BarStyle::Custom)] {
        h.app.settings.settings_theme=theme;h.app.settings.performance_bar=style;
        h.app.settings.performance_metrics=if style==BarStyle::Custom {all_metrics & !Metric::GpuTemp.bit()} else {7};
        for frame in 0..3 {
            let root=h.ctx.run(Default::default(),|ctx|h.app.settings_ui(ctx,false));
            let child=output.lock().unwrap().take().unwrap();
            settings_raster.update_textures(&root.textures_delta);settings_raster.update_textures(&child.textures_delta);
            if frame==2 {
                settings_raster.buf.fill([0.0,0.0,0.0,1.0]);settings_raster.draw(&h.ctx.tessellate(child.shapes,child.pixels_per_point),child.pixels_per_point);
                let mut rgba=Vec::new();settings_raster.to_rgba8(&mut rgba);
                image::save_buffer(dir.join(format!("{name}.png")),&rgba,690,1240,image::ColorType::Rgba8).unwrap();
            }
            settings_raster.free_textures(&root.textures_delta);settings_raster.free_textures(&child.textures_delta);
        }
    }
}

/// 设置页的 AI 额度账号默认收起成一行；需要检查明细的测试先把所有账号展开。


#[cfg(all(windows, feature = "material"))]
#[path = "quota_plugin_ui_tests.rs"]

#[path="module_settings_tests.rs"]
mod module_settings_tests;
#[cfg(feature="sdk-js")]
#[test]
fn local_plugin_island_card_shares_todo_page_and_obeys_module_switch(){
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();h.app.settings.pet=false;
    let sample=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("spike/examples/payday/plugin.json");
    h.app.plugins.install(&sample).unwrap();h.app.plugins.enable("example.payday",true).unwrap();
    h.app.show_page(crate::app::TODO_PAGE);h.hover(H::local(250.,65.),1.);
    let has_text=|h:&H,word:&str|h.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().contains(word)));
    assert!(has_text(&h,"距离发薪"));assert!(has_text(&h,"加一个"));
    if let Some(dest)=std::env::var_os("LINGQI_LOCAL_PREVIEW"){
        let dest=std::path::PathBuf::from(dest);std::fs::create_dir_all(&dest).unwrap();
        let mut raster=feature_raster();crate::theme::load_fonts(&h.ctx);
        shoot_feature(&mut h,&mut raster,&dest,"岛上倒计时",5,Some(H::local(250.,65.)),"2026-10-28");
    }
    h.app.set_page_visible(crate::app::TODO_PAGE,false);h.frame(vec![]);assert!(!h.app.plugins.has_card());
    h.app.set_page_visible(crate::app::TODO_PAGE,true);
    let until=std::time::Instant::now()+std::time::Duration::from_secs(3);
    while !h.app.plugins.has_card(){assert!(std::time::Instant::now()<until);h.frame(vec![]);std::thread::yield_now();}
}

#[cfg(windows)]
#[test]
#[ignore="导出六种性能常驻样式在左侧停靠时的画面，拼成一张对照图"]
fn performance_side_gallery() {
    use crate::{model::Dock,performance::BarStyle};
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dir=std::path::PathBuf::from(std::env::var_os("LINGQI_PERFORMANCE_PREVIEW_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let (w,hgt)=(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H);
    let mut shots=Vec::new();
    for style in BarStyle::ALL {
        let mut h=H::new();h.app.settings.dock=Dock::Left;h.app.settings.pet=false;
        h.app.set_page_visible(crate::app::PERFORMANCE_PAGE,true);
        h.app.settings.performance_bar=style;
        h.app.settings.performance_metrics=0b1_1000_0101;
        let start=std::time::Instant::now();
        while start.elapsed().as_secs()<4 {h.app.performance.tick(true,true,false,&h.ctx);std::thread::sleep(std::time::Duration::from_millis(50));}
        let mut raster=feature_raster_sized(w,hgt);
        for frame in 0..60 {
            h.t+=1.0/60.0;
            let env=Env{t:h.t,clock:"09:41:00".into(),today:"2026-11-11".into(),date:"11月11日 周三".into(),cursor:Some(Pos2::new(400.0,590.0)),..Default::default()};
            let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,
                viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(2.0),..Default::default()})).collect(),..Default::default()};
            let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{
                h.app.frame(ui,Rect::from_min_size(ui.max_rect().min,Vec2::new(w,hgt)),&env);
            });});
            raster.update_textures(&output.textures_delta);
            if frame==59 {
                raster.buf.fill([0.20,0.24,0.32,1.0]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
                let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
                let image=image::RgbaImage::from_raw((w*2.0)as u32,(hgt*2.0)as u32,rgba).unwrap();
                let rect=h.app.island_rect(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt)),h.app.w.pos,h.app.h.pos);
                shots.push(image::imageops::crop_imm(&image,0,((rect.top()-12.0).max(0.0)*2.0)as u32,120,((rect.height()+24.0)*2.0)as u32).to_image());
            }
            raster.free_textures(&output.textures_delta);
        }
    }
    let tall=shots.iter().map(|s|s.height()).max().unwrap();
    let mut sheet=image::RgbaImage::from_pixel(130*shots.len() as u32,tall,image::Rgba([51,61,82,255]));
    for (i,shot) in shots.iter().enumerate() {image::imageops::replace(&mut sheet,shot,130*i as i64,0);}
    sheet.save(dir.join("side-styles.png")).unwrap();
}

#[cfg(windows)]
#[test]
#[ignore="导出综合监测常驻下展开、收起的逐帧画面"]
fn performance_morph_film() {
    let _guard=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dir=std::path::PathBuf::from(std::env::var_os("LINGQI_PERFORMANCE_PREVIEW_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    // LINGQI_FILM_DATA 指向一份设置目录时按那份设置的常驻宽度拍。
    let seed=std::env::var_os("LINGQI_FILM_DATA").map(std::path::PathBuf::from);
    let mut h=H::with_data(seed.as_deref());
    h.app.set_page_visible(crate::app::PERFORMANCE_PAGE,true);
    h.app.settings.performance_bar=match std::env::var("LINGQI_FILM_BAR").as_deref() {
        Ok("pulse")=>crate::performance::BarStyle::Pulse,
        Ok("custom")=>{h.app.settings.performance_metrics=crate::performance::Metric::ALL.iter().fold(0,|bits,m|bits|m.bit());crate::performance::BarStyle::Custom},
        _=>crate::performance::BarStyle::Dashboard,
    };
    h.app.page=crate::app::PERFORMANCE_PAGE;
    for _ in 0..30 {h.frame(vec![]);}
    println!("resident width {} / panel {}",h.app.resident.target.width,PW);
    let mut raster=feature_raster();
    let ppp=2.0;let (w,hgt)=(crate::app::CANVAS_W,WIN_H);
    let today=chrono::Local::now().date_naive().to_string();
    let mut film:Vec<image::RgbaImage>=Vec::new();
    let away=Pos2::new(5.0,WIN_H-5.0);let inside=Pos2::new(WIN_W/2.0,15.0);
    for f in 0..300 {
        let cursor=if f<30 {away} else if f<140 {inside} else {away};
        h.t+=1.0/60.0;
        let env=Env{t:h.t,clock:"09:41:00".into(),today:today.clone(),date:"11月4日 周三".into(),cursor:Some(cursor),..Default::default()};
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,
            events:vec![egui::Event::PointerMoved(cursor)],
            viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(ppp),..Default::default()})).collect(),..Default::default()};
        let app=&mut h.app;
        let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{let win=Rect::from_min_size(ui.max_rect().min,Vec2::new(WIN_W,WIN_H));app.frame(ui,win,&env);});});
        raster.update_textures(&output.textures_delta);
        let primitives=h.ctx.tessellate(output.shapes,output.pixels_per_point);
        if ((30..80).contains(&f)||(140..170).contains(&f))&&f%2==0 {
            raster.buf.fill([0.20,0.24,0.32,1.0]);
            raster.draw(&primitives,output.pixels_per_point);
            let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
            let image=image::RgbaImage::from_raw((w*ppp) as u32,(hgt*ppp) as u32,rgba).unwrap();
            film.push(image::imageops::crop_imm(&image,(WIN_W*ppp/2.0) as u32-600,0,1200,560).to_image());
        }
        raster.free_textures(&output.textures_delta);
    }
    for (n,chunk) in film.chunks(4).enumerate() {
        let mut sheet=image::RgbaImage::new(1200,560*chunk.len() as u32);
        for (i,frame) in chunk.iter().enumerate(){image::imageops::replace(&mut sheet,frame,0,560*i as i64);}
        sheet.save(dir.join(format!("morph-{n}.png"))).unwrap();
    }
}

#[cfg(all(windows, feature = "material"))]
#[path = "plugin_status_ui_tests.rs"]
mod plugin_status_ui_tests;

#[test]
#[ignore="按需导出灵栖岛体预览：LINGQI_PREVIEW=目录"]
fn lingqi_island_gallery(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let dest=std::path::PathBuf::from(std::env::var_os("LINGQI_PREVIEW").expect("需指定预览目录"));
    std::fs::create_dir_all(&dest).unwrap();
    for (name,style,open) in [
        ("island-atlas",crate::model::PetStyle::Atlas,false),
        ("island-generated",crate::model::PetStyle::Generated,false),
        ("island-generated-open",crate::model::PetStyle::Generated,true),
    ] {
        let mut h=H::new();
        h.ctx.set_pixels_per_point(2.0);
        h.app.settings.pet_style=style;
        if open {h.app.open=true;}
        // 先跑几帧，让精灵长出呼吸与视线。
        for _ in 0..40 {h.frame(vec![]);}
        let ppp=h.ctx.pixels_per_point();
        let mut raster=crate::preview_raster::Raster::new((crate::app::CANVAS_W*ppp) as usize,(WIN_H*ppp) as usize);
        raster.update_textures(&egui::TexturesDelta{set:vec![(egui::TextureId::default(),
            egui::epaint::ImageDelta::full(h.ctx.fonts(|f|f.image()),egui::TextureOptions::LINEAR))],..Default::default()});
        raster.buf.fill([0.10,0.12,0.16,1.0]);
        raster.draw(&h.ctx.tessellate(h.shapes.clone(),ppp),ppp);
        let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
        let image=image::RgbaImage::from_raw(raster.w as u32,raster.h as u32,rgba).unwrap();
        let height=if open {h.app.expanded_height()} else {crate::app::PH};
        image::imageops::crop_imm(&image,
            ((WIN_W-crate::app::PW)/2.0*ppp-12.0) as u32,0,
            ((crate::app::PW+12.0)*ppp+12.0) as u32,
            ((height+12.0)*ppp) as u32,
        ).to_image().save(dest.join(format!("{name}.png"))).unwrap();
    }
}



fn shoot_unread(h:&mut H,raster:&mut crate::preview_raster::Raster,dest:&std::path::Path,name:&str,frames:usize,cursor:Option<Pos2>){
    let ppp=2.0;
    let side=h.app.settings.dock.is_side();
    let (w,hgt)=if side {(crate::side_dock::SIDE_W,crate::side_dock::SIDE_H)} else {(crate::app::CANVAS_W,WIN_H)};
    let (ww,wh)=if side {(w,hgt)} else {(WIN_W,WIN_H)};
    for f in 0..frames {
        h.t+=1.0/60.0;
        let env=Env{t:h.t,clock:"09:41:00".into(),today:"2026-11-05".into(),date:"11月5日 周四".into(),cursor,battery:Some(76),..Default::default()};
        let raw=egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,hgt))),time:Some(h.t),focused:true,
            events:cursor.map(|p|vec![egui::Event::PointerMoved(p)]).unwrap_or_default(),
            viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(ppp),..Default::default()})).collect(),..Default::default()};
        let app=&mut h.app;
        let output=h.ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx,|ui|{let win=Rect::from_min_size(ui.max_rect().min,Vec2::new(ww,wh));app.frame(ui,win,&env);});});
        raster.update_textures(&output.textures_delta);
        if f+1==frames {
            raster.buf.fill([0.20,0.24,0.32,1.0]);
            raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);
            let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
            let image=image::RgbaImage::from_raw((w*ppp) as u32,(hgt*ppp) as u32,rgba).unwrap();
            let image=if side {image::imageops::crop_imm(&image,0,240,(w*ppp) as u32,720).to_image()} else {image::imageops::crop_imm(&image,0,0,(WIN_W*ppp) as u32,(WIN_H*ppp) as u32).to_image()};
            image.save(dest.join(format!("{name}.png"))).unwrap();
        }
        raster.free_textures(&output.textures_delta);
    }
}
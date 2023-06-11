use log::error;

use std::{cell::RefCell, pin::Pin, rc::Rc, thread::current};

use crate::{
    appplayer::AppPlayer,
    file_store::{FileStoreError, FileViewNode, FileStore},
    playlist::PlayList,
    TemplateApp,
};
use egui::*;
use egui_extras::{Size, StripBuilder};

pub(crate) fn ui_button_panel(app: &mut TemplateApp, ctx: &egui::Context, ui: &mut Ui) {
    let file_store = &mut app.file_store;
    let current_typed_no = &mut app.current_typed_no;

    let mut rt = RichText::new (current_typed_no.clone());
    rt = rt.font(FontId::proportional(30.0));
    rt = rt.color(Color32::BLUE);

    ui.label(rt);

    // button panel
    StripBuilder::new(ui)
        .sizes(Size::remainder(), 4)
        .vertical(|mut strip| {
            for i in 0..4 {
                strip.cell(|ui| {
                    StripBuilder::new(ui)
                        .sizes(Size::remainder(), 3)
                        .horizontal(|mut strip| {
                            for j in 0..3 {
                                const BACKSPACE: &'static str = "<-";
                                const ENTER: &'static str = "Enter";

                                strip.cell(|ui| {
                                    let mut no = i32::to_string(&(i * 3 + j));
                                    no = match no.as_str() {
                                        "10" => BACKSPACE.into(),
                                        "11" => ENTER.into(),
                                        e => e.into(),
                                    };

                                    let mut b = widgets::Button::new(&no);
                                    b = b.min_size(ui.available_rect_before_wrap().size());

                                    if ui.add(b).clicked() {

                                        match no.as_str() {
                                            BACKSPACE => {
                                                if !current_typed_no.is_empty() {
                                                    *current_typed_no = current_typed_no
                                                        [0..current_typed_no.len() - 1]
                                                        .to_string();
                                                }
                                            }
                                            ENTER => {
                                                // select file

                                                if let Some(view) = &file_store.default_view {
                                                    let result = view.find_first_file();
                                                    if result.is_some() {
                                                        let view_node = result.unwrap();
                                                        let file_node = Rc::clone(&view_node.borrow().node);
                                                        let was_empty = app.appplayer.playlist.file_list.is_empty();
                                                        app.appplayer.playlist.add(&file_node);
                                                        if was_empty && app.appplayer.play_mod {
                                                            app.appplayer.play_file_on_top();
                                                        }

                                                        *current_typed_no = "".into();
                                                    }
                                                }


                                            }

                                            e => {
                                                *current_typed_no =
                                                    format!("{}{}", current_typed_no, e);
                                            }
                                        }

                                        if let Ok(new_view) =
                                            file_store.view(&Some(current_typed_no.clone()))
                                        {
                                            new_view.expand_all();
                                            file_store.default_view = Some(new_view);
                                        } else {
                                            file_store.default_view = None;
                                        }
                                    }
                                });
                            }
                        });
                })
            }
        });
}

pub(crate) fn ui_playlist_right_panel(app: &mut TemplateApp, ctx: &egui::Context, ui: &mut Ui) {
    StripBuilder::new(ui)
        .size(Size::remainder())
        .vertical(|mut strip| {
            strip.strip(|builder| {
                builder.sizes(Size::remainder(), 2).vertical(|mut strip| {
                    strip.cell(|ui| {
                        StripBuilder::new(ui)
                            .size(Size::remainder())
                            .size(Size::Absolute {
                                initial: 30.0,
                                range: (30.0, 60.0),
                            })
                            .vertical(|mut strip| {
                                strip.cell(|ui| {
                                    ui.group(|ui| {
                                        StripBuilder::new(ui).size(Size::remainder()).vertical(
                                            |mut strip| {
                                                strip.cell(|ui| {
                                                    ui.horizontal( |ui| {
                                                        if ui.toggle_value(&mut app.appplayer.play_mod, "Play").clicked() {
                                                            if app.appplayer.play_mod {
                                                                // play
                                                                app.appplayer.play_file_on_top();
                                                            } else {
                                                                app.appplayer.stop();
                                                            }
                                                        }

                                                        ui.label("Currently On AIR ..");
                                                        ui.label(format!("{:.0}s",&app.current_duration.as_secs_f32()));    
                                                       
                                                       
                                                        if ui.button("Next").clicked() {
                                                            app.appplayer.next();
                                                        }

                                                        ui.ctx().request_repaint();
                                                    });
                                                    ui.separator();

                                                    egui::ScrollArea::both().show(ui, |ui| {
                                                        StripBuilder::new(ui)
                                                            .size(Size::remainder())
                                                            .horizontal(|mut strip| {
                                                                strip.cell(|ui| {

                                                                    let isplaying = app.appplayer.is_playing();
                                                                    let mut deleted: Option<usize> = None;
                                                                    for (index,i) in app
                                                                        .appplayer
                                                                        .playlist
                                                                        .file_list.iter_mut().enumerate()
                                                                    {
                                                                        let n = i.borrow();
                                                                        let mut rt = RichText::new (&n.name);
                                                                        if index == 0 {
                                                                            if isplaying {                                                                          
                                                                            
                                                                                    rt = rt.font(FontId::proportional(20.0));
                                                                                    rt = rt.color(Color32::RED);
                                                                                
                                                                            
                                                                            } else {

                                                                            }
                                                                        ui.label(rt);
                                                                       } else {
                                                                        let mut checked = false;
                                                                        if ui.checkbox(&mut checked, &n.name).clicked() {
                                                                            deleted = Some(index);
                                                                        }
                                                                       }
                                                                       
                                                                    }

                                                                    if let Some(index) = deleted {
                                                                        app.appplayer.playlist.file_list.swap_remove(index);
                                                                    }

                                                                });
                                                            });
                                                    });
                                                });
                                            },
                                        );
                                    });
                                });
                                strip.strip(|mut strip| {
                                    strip.size(Size::remainder()).horizontal(|mut strip| {
                                        strip.cell(|ui| {
                                            ui.horizontal(|ui| {
                                                

                                                ui.spacing();
                                               
                                            });
                                        });
                                    });
                                });
                            });
                    });
                    strip.strip(|builder| {
                        builder.sizes(Size::remainder(), 1).vertical(|mut strip| {
                            strip.cell(|ui| {
                                ui_button_panel(app, ctx, ui);
                            });
                        });
                    });
                });
            });
        });
}

/// recursive function to display files
fn display_tree(
    appplayer: &mut AppPlayer,
    number_selected: &mut String,
    files_folder: &mut Rc<RefCell<FileViewNode>>,
    ui: &mut Ui,
) -> Result<bool, FileStoreError> {

    let mut file_selected = false;
    let mut bfile_folder = files_folder.borrow_mut();
    for mut ele in bfile_folder.childs.iter_mut() {
        let node_is_folder;
        let element_name;
        {
            let bele = &ele.borrow_mut();
            element_name = bele.name().clone();
            let node = &bele.node;

            let bnode = node.borrow();
            node_is_folder = bnode.is_folder;
        }

        if node_is_folder {
            let default_opened: bool;
            {
                default_opened = ele.borrow().expanded;
            }
            let r = CollapsingHeader::new(&element_name)
                //.default_open(default_opened)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    match display_tree(appplayer,  number_selected, &mut ele, ui) {
                        Err(e) =>  {
                                error!("error in displaying sub tree {}", e);
                        },
                        Ok(returned_value) => {file_selected = true}
                }
                });

            // let r = ui.add(ch);
            let bele = &mut ele.borrow_mut();
            bele.expanded = r.fully_open();
        } else {
            let clicked: bool;
            {
                let mut bele = ele.borrow_mut();
                if ui.checkbox(&mut bele.selected, element_name).clicked() {
                    clicked = true;
                    // reset the selected point
                    bele.selected = false;
                } else {
                    clicked = false;
                }
            }
            if clicked {
                appplayer.playlist.add_fileviewnode(ele);
                if appplayer.play_mod {
                    if !appplayer.is_playing() {
                        appplayer.play_file_on_top();
                    }
                }
                *number_selected = "".into();
                file_selected = true;
                
            }
        }
    }
    Ok(file_selected)
}

pub(crate) fn ui_content(app: &mut TemplateApp, ctx: &egui::Context, ui: &mut Ui) {
    let dark_mode = ui.visuals().dark_mode;
    let faded_color = ui.visuals().window_fill();
    let _faded_color = |color: Color32| -> Color32 {
        let t = if dark_mode { 0.95 } else { 0.8 };
        egui::lerp(Rgba::from(color)..=Rgba::from(faded_color), t).into()
    };

    StripBuilder::new(ui)
        .size(Size::relative(0.5))
        .size(Size::relative(0.5))
        .horizontal(|mut strip| {
            strip.cell(|ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    StripBuilder::new(ui)
                        .size(Size::remainder())
                        .horizontal(|mut strip| {
                            strip.cell(|ui| {

                                let filestore = &mut app.file_store;
                                if let Some(view) = &mut filestore.default_view {
                                    
                                    match display_tree(&mut app.appplayer,  &mut app.current_typed_no, &mut view.root, ui) {
                                        Err(e) =>
                                        
                                        {
                                            error!("error in display tree: {}", e);
                                        },
                                        Ok(returned_value) => {
                                            if returned_value {
                                                    if let Ok(new_view) =
                                                    filestore.view(&Some(app.current_typed_no.clone()))
                                                {
                                                    new_view.expand_all();
                                                    filestore.default_view = Some(new_view);
                                                } else {
                                                    filestore.default_view = None;
                                                }
                                            }
                                        }
    
                                    } 
                                    





                                    
                                } else {
                                    ui.label("no files");
                                }
                            });
                        });
                });
            });
            strip.cell(|ui| {
                ui_playlist_right_panel(app, ctx, ui);
            });
        });
}

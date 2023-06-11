use log::error;

use std::{cell::RefCell, pin::Pin, rc::Rc, thread::current};

use crate::{
    appplayer::AppPlayer,
    file_store::{FileStoreError, FileViewNode},
    playlist::PlayList,
    TemplateApp,
};
use egui::*;
use egui_extras::{Size, StripBuilder};

pub(crate) fn ui_button_panel(app: &mut TemplateApp, ctx: &egui::Context, ui: &mut Ui) {
    let file_store = &mut app.file_store;
    let current_typed_no = &mut app.current_typed_no;

    ui.label(current_typed_no.as_str());

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
                                        println!("clicked");
                                        // button clicked
                                        match no.as_str() {
                                            BACKSPACE => {
                                                println!("clicked on backspace");
                                                if !current_typed_no.is_empty() {
                                                    *current_typed_no = current_typed_no
                                                        [0..current_typed_no.len() - 1]
                                                        .to_string();
                                                }
                                            }
                                            ENTER => {
                                                // select file
                                                println!("clicked on enter");
                                            }

                                            e => {
                                                println!("clicked on button {}", e);
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
                                                        ui.label("Currently On AIR ..");
                                                        ui.label(format!(" - {:?}",&app.current_duration));    
                                                    });
                                                    ui.separator();

                                                    egui::ScrollArea::both().show(ui, |ui| {
                                                        StripBuilder::new(ui)
                                                            .size(Size::remainder())
                                                            .horizontal(|mut strip| {
                                                                strip.cell(|ui| {

                                                                    let isplaying = app.appplayer.is_playing();
                                                                    for (index,i) in app
                                                                        .appplayer
                                                                        .playlist
                                                                        .file_list.iter_mut().enumerate()
                                                                    {
                                                                        let n = i.borrow();
                                                                        let mut rt = RichText::new (&n.name);
                                                                        if  isplaying {                                                                          
                                                                           if index == 0 {
                                                                                rt = rt.font(FontId::proportional(30.0));
                                                                                rt = rt.color(Color32::RED);
                                                                           }
                                                                        }
                                                                       ui.label(rt);
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
                                                if ui
                                                    .toggle_value(
                                                        &mut app.appplayer.play_mod,
                                                        "Play",
                                                    )
                                                    .clicked()
                                                {
                                                    if app.appplayer.play_mod {
                                                        // play
                                                        app.appplayer.play_file_on_top();
                                                    } else {
                                                        app.appplayer.stop();
                                                    }
                                                }

                                                ui.spacing();
                                                if ui.button("Next").clicked() {
                                                    app.appplayer.next();

                                                    // inform the element has been stopped to player
                                                }
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
    app_player: &mut AppPlayer,
    files_folder: &mut Rc<RefCell<FileViewNode>>,
    ui: &mut Ui,
) -> Result<(), FileStoreError> {
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

                    if let Err(e) = display_tree(app_player, &mut ele, ui) {
                        error!("error in displaying sub tree {}", e);
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
                app_player.playlist.add_fileviewnode(ele);
                if app_player.play_mod {
                    if !app_player.is_playing() {
                        app_player.play_file_on_top();
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn ui_content(app: &mut TemplateApp, ctx: &egui::Context, ui: &mut Ui) {
    let dark_mode = ui.visuals().dark_mode;
    let faded_color = ui.visuals().window_fill();
    let faded_color = |color: Color32| -> Color32 {
        use egui::Rgba;
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
                                if let Some(view) = &mut app.file_store.default_view {
                                    if let Err(e) =
                                        display_tree(&mut app.appplayer, &mut view.root, ui)
                                    {
                                        error!("error in display tree: {}", e);
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

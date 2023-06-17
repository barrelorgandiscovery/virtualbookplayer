use log::error;

use std::{cell::RefCell, rc::Rc};

use chrono::Local;

use crate::{
    appplayer::AppPlayer,
    file_store::{FileStore, FileStoreError, FileViewNode},
    playlist, VirtualBookApp,
};
use egui::*;
use egui_extras::{Size, StripBuilder};

pub const BACKSPACE: &'static str = "<-";
pub const ENTER: &'static str = "Enter";

pub fn handling_key(
    no: &String,
    current_typed_no: &mut String,
    file_store: &mut Option<FileStore>,
    appplayer: &mut AppPlayer,
) {
    match no.as_str() {
        BACKSPACE => {
            if !current_typed_no.is_empty() {
                *current_typed_no = current_typed_no[0..current_typed_no.len() - 1].to_string();
            }
        }
        ENTER => {
            // select file
            if let Some(filestore) = &file_store {
                if let Some(view) = &filestore.default_view {
                    let result = view.find_first_file();
                    if result.is_some() {
                        let view_node = result.unwrap();
                        let file_node = Rc::clone(&view_node.borrow().node);
                        let was_empty = appplayer.playlist.file_list.is_empty();

                        appplayer
                            .playlist
                            .add_from_path_and_expand_playlists(&file_node.borrow().path);

                        if was_empty && appplayer.play_mod {
                            appplayer.play_file_on_top();
                        }

                        *current_typed_no = "".into();
                    }
                }
            }
        }

        e => {
            *current_typed_no = format!("{}{}", current_typed_no, e);
        }
    }

    if let Some(filestore) = file_store {
        if let Ok(new_view) = filestore.view(&Some(current_typed_no.clone())) {
            new_view.expand();
            filestore.default_view = Some(new_view);
        } else {
            filestore.default_view = None;
        }
    }
}

pub(crate) fn ui_button_panel(app: &mut VirtualBookApp, ctx: &egui::Context, ui: &mut Ui) {
    let mut file_store = &mut app.file_store;
    let current_typed_no = &mut app.current_typed_no;

    let messages_i18n = &app.i18n;

    // button panel
    StripBuilder::new(ui)
        .sizes(Size::remainder(), 5)
        .vertical(|mut strip| {
            strip.cell(|ui| {
                let mut rt = RichText::new(current_typed_no.clone());
                rt = rt.font(FontId::proportional(30.0));
                rt = rt.color(ui.style().visuals.selection.stroke.color);
                ui.label(rt);
            });

            for i in 0..4 {
                strip.cell(|ui| {
                    StripBuilder::new(ui)
                        .sizes(Size::remainder(), 3)
                        .horizontal(|mut strip| {
                            for j in 0..3 {
                                strip.cell(|ui| {
                                    let mut no = i32::to_string(&(i * 3 + j));
                                    no = match no.as_str() {
                                        "10" => BACKSPACE.into(),
                                        "11" => ENTER.into(),
                                        e => e.into(),
                                    };

                                    let mut b = widgets::Button::new(&no);
                                    b = b.min_size(ui.available_rect_before_wrap().size());

                                    ui.centered_and_justified(|ui| {
                                        if ui.add(b).clicked() {

                                            // Handling NO
                                            handling_key(&no, current_typed_no, file_store, &mut app.appplayer);
                                        }
                                    });
                                });
                            }
                        });
                })
            }
        });
}

pub(crate) fn ui_playlist_right_panel(app: &mut VirtualBookApp, ctx: &egui::Context, ui: &mut Ui) {
    StripBuilder::new(ui)
        .size(Size::remainder())
        .vertical(|mut strip| {
            strip.strip(|builder| {
                builder.sizes(Size::remainder(), 2).vertical(|mut strip| {
                    strip.cell(|ui| {
                        StripBuilder::new(ui)
                            .size(Size::remainder())
                            .vertical(|mut strip| {
                                strip.cell(|ui| {
                                    ui.group(|ui| {
                                        StripBuilder::new(ui).size(Size::remainder()).vertical(
                                            |mut strip| {
                                                strip.cell(|ui| {
                                                    ui.horizontal(|ui| {
                                                        if ui.button(&app.i18n.next).clicked() {
                                                            app.appplayer.next();
                                                        }

                                                        if let Some(path_buf) = &app.file_store_path
                                                        {
                                                            if ui
                                                                .button(&app.i18n.save_playlist)
                                                                .clicked()
                                                            {
                                                                let date = Local::now();

                                                                let formatted_date = date
                                                                    .format("%Y-%m-%d_%H-%M-%S");

                                                                let mut pb = path_buf.clone();

                                                                pb.push(format!(
                                                                    "playlist_{}.playlist",
                                                                    formatted_date
                                                                ));
                                                                playlist::save(
                                                                    &app.appplayer.playlist,
                                                                    &pb,
                                                                );
                                                            }
                                                        }
                                                    });

                                                    ui.separator();

                                                    egui::ScrollArea::both().show(ui, |ui| {
                                                        StripBuilder::new(ui)
                                                            .size(Size::remainder())
                                                            .horizontal(|mut strip| {
                                                                strip.cell(|ui| {
                                                                    let isplaying =
                                                                        app.appplayer.is_playing();
                                                                    let mut deleted: Option<usize> =
                                                                        None;
                                                                    for (index, i) in app
                                                                        .appplayer
                                                                        .playlist
                                                                        .file_list
                                                                        .iter_mut()
                                                                        .enumerate()
                                                                    {
                                                                        if !(isplaying
                                                                            && index == 0)
                                                                        {
                                                                            let mut checked = false;
                                                                            if ui
                                                                                .checkbox(
                                                                                    &mut checked,
                                                                                    &i.name,
                                                                                )
                                                                                .clicked()
                                                                            {
                                                                                deleted =
                                                                                    Some(index);
                                                                            }
                                                                        }
                                                                    }

                                                                    if let Some(index) = deleted {
                                                                        app.appplayer
                                                                            .playlist
                                                                            .file_list
                                                                            .remove(index);
                                                                    }
                                                                });
                                                            });
                                                    });
                                                });
                                            },
                                        );
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
    for mut element in bfile_folder.childs.iter_mut() {
        let node_is_folder;
        let element_name;
        {
            let borrowed_element = &element.borrow_mut();
            element_name = borrowed_element.name().clone();
            let node = &borrowed_element.node;

            let bnode = node.borrow();
            node_is_folder = bnode.is_folder;
        }

        if node_is_folder {
            let default_opened: bool;
            let mut clicked = None;
            {
                let e = element.borrow();
                default_opened = e.expanded;
                clicked = e.clicked_for_open;
            }

            let r = CollapsingHeader::new(&element_name)
                // .default_open(None)
                .open(clicked)
                .default_open(default_opened)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    match display_tree(appplayer, number_selected, &mut element, ui) {
                        Err(e) => {
                            error!("error in displaying sub tree {}", e);
                            // continue
                        }
                        Ok(_returned_value) => file_selected = true,
                    }
                });
            let borrowed_element = &mut element.borrow_mut();
            borrowed_element.clicked_for_open = None;

            if r.header_response.clicked() {
                borrowed_element.expanded = default_opened ^ true;
                borrowed_element.clicked_for_open = Some(borrowed_element.expanded);
            }
        } else {
            // file and not a folder
            let clicked: bool;
            {
                let mut bele = element.borrow_mut();
                if ui.checkbox(&mut bele.selected, element_name).clicked() {
                    clicked = true;
                    // reset the selected point
                    bele.selected = false;
                } else {
                    clicked = false;
                }
            }
            if clicked {
                appplayer
                    .playlist
                    .add_fileviewnode_and_read_playlists(element);
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

pub(crate) fn ui_content(app: &mut VirtualBookApp, ctx: &egui::Context, ui: &mut Ui) {
    // let dark_mode = ui.visuals().dark_mode;
    // let faded_color = ui.visuals().window_fill();
    // let _faded_color = |color: Color32| -> Color32 {
    //     let t = if dark_mode { 0.95 } else { 0.8 };
    //     egui::lerp(Rgba::from(color)..=Rgba::from(faded_color), t).into()
    // };

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
                                if let Some(filestore) = &mut app.file_store {
                                    if let Some(view) = &mut filestore.default_view {
                                        match display_tree(
                                            &mut app.appplayer,
                                            &mut app.current_typed_no,
                                            &mut view.root,
                                            ui,
                                        ) {
                                            Err(e) => {
                                                error!("error in display tree: {}", e);
                                            }
                                            Ok(returned_value) => {
                                                if returned_value {
                                                    if let Ok(new_view) = filestore
                                                        .view(&Some(app.current_typed_no.clone()))
                                                    {
                                                        new_view.expand();
                                                        filestore.default_view = Some(new_view);
                                                    } else {
                                                        filestore.default_view = None;
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        ui.label(&app.i18n.aucun_fichiers);
                                    }
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

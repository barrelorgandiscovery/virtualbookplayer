use log::{debug, error};

use std::{cell::RefCell, rc::Rc};

use chrono::Local;

use crate::{
    appplayer::AppPlayer,
    file_store::{FileStore, FileStoreError, FileViewNode},
    playlist,
    virtualbookcomponent::VirtualBookComponent,
    VirtualBookApp,
};
use egui::*;
use egui_dnd::dnd;
use egui_extras::{Size, StripBuilder};

use super::Screen;

pub const BACKSPACE: &str = "<-";
pub const ENTER: &str = "Enter";

pub fn handling_key(
    no: &str,
    current_typed_no: &mut String,
    file_store: &mut Option<FileStore>,
    appplayer: &mut AppPlayer,
    extensions_filter: &Option<Vec<String>>,
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

                    if let Some(view_node) = result {
                        let file_node = Rc::clone(&view_node.borrow().node);
                        let was_empty;
                        {
                            let mut locked_playlist = appplayer
                                .playlist
                                .lock()
                                .expect("fail to lock the playlist");
                            was_empty = locked_playlist.file_list.is_empty();

                            locked_playlist
                                .add_from_path_and_expand_playlists(&file_node.borrow().path);
                        }
                        if was_empty && appplayer.play_mod {
                            appplayer.play_file_on_top();
                        }

                        *current_typed_no = "".into();
                    }
                }
            }
        }
        "Escape" => {
            *current_typed_no = "".into();
        }
        "Space" => {
            *current_typed_no += " ";
        }
        e => {
            *current_typed_no = format!("{}{}", current_typed_no, e);
        }
    }

    // filtering the treeview
    if let Some(filestore) = file_store {
        if let Ok(new_view) = filestore.view(&Some(current_typed_no.clone()), extensions_filter) {
            new_view.expand();
            filestore.default_view = Some(new_view);
        } else {
            filestore.default_view = None;
        }
    }
}

pub(crate) fn ui_button_panel(app: &mut VirtualBookApp, _ctx: &egui::Context, ui: &mut Ui) {
    let file_store = &mut app.file_store;
    let current_typed_no = &mut app.current_typed_no;

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
                                            handling_key(
                                                &no,
                                                current_typed_no,
                                                file_store,
                                                &mut app.appplayer,
                                                &app.extensions_filters,
                                            );
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
                builder.sizes(Size::remainder(), if app.hidden_number_pad { 1 } else { 2 } ).vertical(|mut strip| {
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

                                                        let appplayer = &mut app.appplayer;
                                                        ui.add_enabled_ui(!appplayer.is_playlist_empty() , |ui| {
                                                            // let play_mod = &mut appplayer.play_mod;
                                                            // if ui
                                                            //     .toggle_value(play_mod, RichText::new('\u{F04B}').color(Color32::GREEN)
                                                            //     .font(FontId::new(26.0, FontFamily::Name("icon_font".into())))
                                                            // ).on_hover_text(&app.i18n.play)
                                                            //     .clicked()
                                                            // {
                                                            //     if *play_mod {
                                                            //         appplayer.play_file_on_top();
                                                            //     } else {
                                                            //         appplayer.stop();
                                                            //     }
                                                            // }

                                                            ui.label(RichText::new(format!("{} {}", egui_phosphor::regular::FILES ,"PlayList : ")).heading());

                                                            if  crate::app::font_button(ui, '\u{23E9}')
                                                                .on_hover_text(&app.i18n.go_to_next_file)
                                                                .clicked() {
                                                                appplayer.next();
                                                            }
                                                        });
                                                        if let Some(path_buf) = &app.file_store_path {
                                                                ui.separator();
                                                                if ui.button( egui_phosphor::regular::LIST_PLUS)
                                                                    .on_hover_text(&app.i18n.save_playlist)
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
                                                                    let locked_playlist = appplayer.playlist.lock().expect("fail to lock the playlist");

                                                                    if let Err(e) = playlist::save(
                                                                        &locked_playlist,
                                                                        &pb,
                                                                    ) {
                                                                        error!("error in saving playlist in {}, {}", pb.display(), e);
                                                                    }
                                                                }
                                                            }
                                                        });

                                                    ui.separator();

                                                    // playlist
                                                    egui::ScrollArea::both().show(ui, |ui| {
                                                        StripBuilder::new(ui)
                                                            .size(Size::remainder())
                                                            .horizontal(|mut strip| {
                                                                strip.cell(|ui| {
                                                                    let isplaying =
                                                                        app.appplayer.is_playing();
                                                                    let mut locked_playlist = app
                                                                    .appplayer
                                                                    .playlist.lock().expect("fail to lock playlist");

                                                                    let mut working_list = locked_playlist.file_list.clone();
                                                                    if isplaying && !working_list.is_empty() {
                                                                        working_list = working_list[1..].to_vec();
                                                                    }

                                                                    let mut deleted: Option<usize> = None;
                                                                    let item_size =  Vec2::new(ui.available_width(), 32.0);
                                                                    // let items = &mut locked_playlist
                                                                    // .file_list;

                                                                    // see https://github.com/lucasmerlin/hello_egui/blob/main/fancy-example/src/main.rs
                                                                    // for dnd examples
                                                                    let response = dnd(ui, "playlist_dnd")
                                                                        .show_custom(|ui, iter| {
                                                                            working_list.iter_mut().enumerate().for_each(|(index, item)| {
                                                                                     iter.next(ui, Id::new(&item), index, true, |ui, item_handle| {
                                                                                        item_handle.ui_sized(ui, item_size, |ui, handle, _state| {
                                                                                            ui.horizontal_wrapped(|ui| {
                                                                                                handle.ui_sized(ui, item_size, |ui| {
                                                                                                        ui.add(
                                                                                                            Label::new(format!("{}:", index + 1))
                                                                                                        );
                                                                                                        ui.label(&item.name);
                                                                                                        if ui.button( egui_phosphor::regular::TRASH).on_hover_text(&app.i18n.button_remove)
                                                                                                            .on_hover_text(&app.i18n.remove_file_from_list)
                                                                                                            .clicked() {
                                                                                                            deleted =
                                                                                                            Some(index);
                                                                                                        }
                                                                                                        ui.end_row();
                                                                                                        ui.separator();
                                                                                                });

                                                                                            });
                                                                                        })
                                                                                    });
                                                                                });
                                                                        }
                                                                    );
                                                                    response.update_vec(&mut working_list);
                                                                    if isplaying && (!locked_playlist.file_list.is_empty()) {
                                                                        locked_playlist.file_list.truncate(1);
                                                                        locked_playlist.file_list.extend(working_list);
                                                                    } else {
                                                                        locked_playlist.file_list = working_list;
                                                                    }

                                                                    if let Some(reason) = response.cancellation_reason() {
                                                                        debug!("Drag has been cancelled because of {:?}", reason);
                                                                    }

                                                                    if let Some(index) = deleted {
                                                                        locked_playlist
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

                    if !app.hidden_number_pad {
                        strip.strip(|builder| {
                            builder.sizes(Size::remainder(), 1).vertical(|mut strip| {
                                strip.cell(|ui| {
                                    ui_button_panel(app, ctx, ui);
                                });
                            });

                        });
                    }
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
    for element in bfile_folder.childs.iter_mut() {
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
            let clicked;
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

                    match display_tree(appplayer, number_selected, element, ui) {
                        Err(e) => {
                            error!("error in displaying sub tree {}", e);
                            // continue
                        }
                        Ok(returned_value) => {
                            if returned_value {
                                file_selected = true
                            }
                        }
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
                {
                    let mut locked_playlist = appplayer
                        .playlist
                        .lock()
                        .expect("fail to lock the playlist");
                    locked_playlist.add_fileviewnode_and_read_playlists(element);
                }
                if appplayer.play_mod && !appplayer.is_playing() {
                    appplayer.play_file_on_top();
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
                        .size(Size::initial(6.0))
                        .size(Size::remainder())
                        .vertical(|mut strip| {
                            strip.cell(|ui| {
                                ui.with_layout(egui::Layout::right_to_left(Align::Max), |ui| {
                                    if !app.current_typed_no.is_empty() {
                                        let s = app.i18n.filter.clone()
                                            + &format!(" : {}", app.current_typed_no);
                                        ui.group(|ui| ui.label(s.as_str()));
                                    }
                                });
                            });
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
                                                    // click detected
                                                    debug!("click on the file, refresh the view");
                                                    if let Ok(new_view) = filestore.view(
                                                        &Some(app.current_typed_no.clone()),
                                                        &app.extensions_filters,
                                                    ) {
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
                StripBuilder::new(ui)
                    .size(Size::initial(12.0))
                    .size(Size::initial(100.0))
                    .size(Size::remainder())
                    .vertical(|mut strip| {
                        strip.cell(|ui| {
                            // name of the element
                            ui.vertical_centered(|ui| {
                                if app.appplayer.is_playing() {
                                    let cell = &app
                                        .appplayer
                                        .playlist
                                        .lock()
                                        .expect("fail to lock playlist")
                                        .current();
                                    match cell {
                                        Some(t) => {
                                            let name = t.name.clone();
                                            let mut rt = RichText::new(format!(" ➡ {} ⬅ ", name));

                                            rt = rt.background_color(
                                                ui.style().visuals.selection.bg_fill,
                                            );
                                            rt =
                                                rt.color(ui.style().visuals.selection.stroke.color);

                                            ui.horizontal(|ui| {
                                                ui.label(rt.monospace());
                                                ui.label(format!(
                                                    "{:.0}s",
                                                    &app.current_duration.as_secs_f32()
                                                ));
                                            });
                                        }
                                        None => {}
                                    }
                                }
                            });
                        });

                        strip.cell(|ui| {
                            // draw book vignette
                            let foffset: f32 = app.pid_regulated_offset as f32;

                            // display virtualbook
                            let mut c = VirtualBookComponent::from_some_virtualbook(
                                app.appplayer.virtual_book.clone(),
                            )
                            .offset(foffset)
                            .xscale(app.xscale)
                            .hide_scrollbar();

                            if c.ui_content(ui).clicked() {
                                app.screen = Screen::Display;
                            }
                        });
                        strip.cell(|ui| {
                            // render playlist panel
                            ui_playlist_right_panel(app, ctx, ui);
                        });
                    });
            });
        });
}

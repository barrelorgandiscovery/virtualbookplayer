use log::{debug, error};

use std::{cell::RefCell, rc::Rc};

use chrono::Local;

use crate::{
    appplayer::AppPlayer,
    duration_to_mm_ss,
    file_store::{FileStore, FileStoreError, FileViewNode},
    playlist,
    virtualbookcomponent::VirtualBookComponent,
    VirtualBookApp,
};
use egui::{epaint::Shadow, *};
use egui_dnd::{dnd, DragDropConfig};
use egui_extras::{Size, StripBuilder};

use super::Screen;

pub const BACKSPACE: &str = "<-";
pub const ENTER: &str = "Enter";

/// Handle Enter key to select and add file to playlist
fn handle_enter_key(
    current_typed_no: &mut String,
    file_store: &Option<FileStore>,
    appplayer: &mut AppPlayer,
) {
    if let Some(filestore) = file_store {
        let current_view = if current_typed_no.is_empty() {
            &filestore.default_view
        } else {
            &filestore.filtered_view
        };

        if let Some(view) = current_view {
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

/// Update file store filtered view based on current typed number
fn update_file_store_filter(
    file_store: &mut Option<FileStore>,
    current_typed_no: &String,
    extensions_filter: &Option<Vec<String>>,
) {
    if let Some(filestore) = file_store {
        if let Ok(mut new_view) = filestore.view(&Some(current_typed_no.clone()), extensions_filter)
        {
            new_view.recurse_expand_first();
            filestore.filtered_view = Some(new_view);
        }
    }
}

#[cfg_attr(any(feature = "profiling"), profiling::function)]
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
            handle_enter_key(current_typed_no, file_store, appplayer);
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
    update_file_store_filter(file_store, current_typed_no, extensions_filter);
}

/// Get button text for number pad based on position
fn get_button_text(row: usize, col: usize) -> String {
    let num = row * 3 + col;
    match num {
        10 => BACKSPACE.into(),
        11 => ENTER.into(),
        _ => num.to_string(),
    }
}

#[cfg_attr(any(feature = "profiling"), profiling::function)]
pub(crate) fn ui_button_panel(app: &mut VirtualBookApp, _ctx: &egui::Context, ui: &mut Ui) {
    let file_store = &mut app.file_store;
    let current_typed_no = &mut app.current_typed_no;

    // button panel - fill all available space
    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
        StripBuilder::new(ui)
            .size(Size::exact(40.0)) // Input field - fixed height
            .sizes(Size::remainder(), 4) // 4 rows - share remaining space equally
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
                                        let button_text = get_button_text(i, j);
                                        let available_size = ui.available_rect_before_wrap().size();
                                        let mut b = widgets::Button::new(&button_text);
                                        b = b.min_size(available_size);

                                        ui.centered_and_justified(|ui| {
                                            if ui.add(b).clicked() {
                                                handling_key(
                                                    &button_text,
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
    });
}

/// Render playlist header with controls (play, next, save)
fn render_playlist_header(app: &mut VirtualBookApp, ui: &mut Ui) {
    let appplayer = &mut app.appplayer;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!appplayer.is_playlist_empty(), |ui| {
            ui.label(
                RichText::new(format!("{} {}", egui_phosphor::regular::FILES, "PlayList : "))
                    .heading(),
            );

            let play_mod = &appplayer.play_mod;
            if !*play_mod
                && ui.button(egui_phosphor::regular::PLAY)
                    .on_hover_text_at_pointer(&app.i18n.go_to_next_file)
                    .clicked()
            {
                appplayer.play_file_on_top();
            }

            if ui.button(egui_phosphor::regular::FAST_FORWARD)
                .on_hover_text_at_pointer(&app.i18n.go_to_next_file)
                .clicked()
            {
                appplayer.next();
            }
        });

        if let Some(path_buf) = &app.file_store_path {
            ui.separator();
            if ui.button(egui_phosphor::regular::LIST_PLUS)
                .on_hover_text_at_pointer(&app.i18n.save_playlist)
                .clicked()
            {
                let date = Local::now();
                let formatted_date = date.format("%Y-%m-%d_%H-%M-%S");
                let mut pb = path_buf.clone();
                pb.push(format!("playlist_{}.playlist", formatted_date));
                let locked_playlist = appplayer
                    .playlist
                    .lock()
                    .expect("fail to lock the playlist");

                if let Err(e) = playlist::save(&locked_playlist, &pb) {
                    error!("error in saving playlist in {}, {}", pb.display(), e);
                }
            }
        }
    });
}

/// Truncate display name if too long
fn truncate_display_name(name: &str, max_length: usize) -> String {
    if name.len() > max_length {
        String::from(name.char_range(0..max_length)) + "..."
    } else {
        name.to_string()
    }
}

/// Render a single playlist item
fn render_playlist_item(
    ui: &mut Ui,
    item: &playlist::PlaylistElement,
    index: usize,
    item_size: Vec2,
    app: &VirtualBookApp,
) -> bool {
    let mut deleted = false;
    ui.horizontal(|ui| {
        ui.spacing();
        if ui.button(egui_phosphor::regular::TRASH)
            .on_hover_text_at_pointer(&app.i18n.remove_file_from_list)
            .clicked()
        {
            deleted = true;
        }
        ui.add(Label::new(format!("{}:", index + 1)));

        // compute size to strip the label
        let mean_displayed_letter = (item_size.x / 10.0) as usize;
        let display_compute_name = truncate_display_name(&item.name, mean_displayed_letter);

        ui.label(&display_compute_name)
            .on_hover_text_at_pointer(&item.name);

        if let Some(additional_informations) = &item.additional_informations {
            if let Some(duration) = additional_informations.duration {
                ui.label(duration_to_mm_ss(&duration));
            }
        }
    });
    deleted
}

/// Render playlist items with drag-and-drop support
fn render_playlist_items(app: &mut VirtualBookApp, ui: &mut Ui) {
    let isplaying = app.appplayer.is_playing();
    let mut locked_playlist = app
        .appplayer
        .playlist
        .lock()
        .expect("fail to lock playlist");

    let mut working_list = locked_playlist.file_list.clone();
    if isplaying && !working_list.is_empty() {
        working_list = working_list[1..].to_vec();
    }

    let mut deleted: Option<usize> = None;
    let item_size = Vec2::new(ui.available_width(), 32.0);

    // see https://github.com/lucasmerlin/hello_egui/blob/main/fancy-example/src/main.rs
    // for dnd examples
    let response = dnd(ui, "playlist_dnd")
        .with_mouse_config(DragDropConfig::touch_scroll())
        .show_custom(|ui, iter| {
            working_list
                .iter_mut()
                .enumerate()
                .for_each(|(index, item)| {
                    #[allow(clippy::needless_borrows_for_generic_args)]
                    iter.next(ui, Id::new(&item), index, true, |ui, item_handle| {
                        item_handle.ui_sized(ui, item_size, |ui, handle, _state| {
                            ui.vertical_centered_justified(|ui| {
                                handle.ui_sized(ui, item_size, |ui| {
                                    if render_playlist_item(ui, item, index, item_size, app) {
                                        deleted = Some(index);
                                    }
                                    ui.end_row();
                                });
                                ui.separator();
                            });
                        })
                    });
                });
        });

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
        let toremove = if isplaying { index + 1 } else { index };
        locked_playlist.file_list.remove(toremove);
    }
}

#[cfg_attr(any(feature = "profiling"), profiling::function)]
pub(crate) fn ui_playlist_right_panel(app: &mut VirtualBookApp, ctx: &egui::Context, ui: &mut Ui) {
    StripBuilder::new(ui)
        .size(Size::remainder())
        .vertical(|mut strip| {
            strip.strip(|builder| {
                builder.sizes(Size::remainder(), if app.hidden_number_pad { 1 } else { 2 }).vertical(|mut strip| {
                    strip.cell(|ui| {
                        StripBuilder::new(ui)
                            .size(Size::remainder())
                            .vertical(|mut strip| {
                                strip.cell(|ui| {
                                    ui.group(|ui| {
                                        StripBuilder::new(ui)
                                            .size(Size::remainder())
                                            .vertical(|mut strip| {
                                                strip.cell(|ui| {
                                                    render_playlist_header(app, ui);
                                                    ui.separator();

                                                    // playlist
                                                    egui::ScrollArea::both().show(ui, |ui| {
                                                        StripBuilder::new(ui)
                                                            .size(Size::remainder())
                                                            .horizontal(|mut strip| {
                                                                strip.cell(|ui| {
                                                                    render_playlist_items(app, ui);
                                                                });
                                                            });
                                                    });
                                                });
                                            });
                                    });
                                });
                            });
                    });

                    if !app.hidden_number_pad {
                        strip.cell(|ui| {
                            ui_button_panel(app, ctx, ui);
                        });
                    }
                });
            });
        });
}

/// Handle folder display in tree view
fn display_folder(
    appplayer: &mut AppPlayer,
    number_selected: &mut String,
    element: &mut Rc<RefCell<FileViewNode>>,
    element_name: String,
    ui: &mut Ui,
) -> Result<bool, FileStoreError> {
    let expanded = {
        let e = element.borrow();
        e.expanded
    };

    let id_source_folder = number_selected.clone() + element_name.as_str();
    let mut file_selected = false;
    let r = CollapsingHeader::new(&element_name)
        .id_source(id_source_folder)
        .open(Some(expanded))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            match display_tree(appplayer, number_selected, element, ui) {
                Err(e) => {
                    error!("error in displaying sub tree {}", e);
                }
                Ok(returned_value) => {
                    if returned_value {
                        file_selected = true;
                    }
                }
            }
        });

    let mut borrowed_element = element.borrow_mut();
    if r.header_response.clicked() {
        borrowed_element.expanded ^= true;
    }

    Ok(file_selected)
}

/// Handle file display in tree view
fn display_file(
    appplayer: &mut AppPlayer,
    number_selected: &mut String,
    element: &mut Rc<RefCell<FileViewNode>>,
    element_name: String,
    ui: &mut Ui,
) -> bool {
    let clicked = {
        let mut bele = element.borrow_mut();
        if ui.checkbox(&mut bele.selected, &element_name).clicked() {
            bele.selected = false;
            true
        } else {
            false
        }
    };

    if clicked {
        {
            let mut locked_playlist = appplayer
                .playlist
                .lock()
                .expect("fail to lock the playlist");
            locked_playlist.add_fileviewnode_and_read_playlists(element);
        }

        // when just added and play is active, play it
        if appplayer.play_mod && !appplayer.is_playing() {
            appplayer.play_file_on_top();
        }

        *number_selected = "".into();
        true
    } else {
        false
    }
}

#[cfg_attr(any(feature = "profiling"), profiling::function)]
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
        let (node_is_folder, element_name) = {
            let borrowed_element = &element.borrow_mut();
            let name = borrowed_element.name().clone();
            let node = &borrowed_element.node;
            let bnode = node.borrow();
            (bnode.is_folder, name)
        };

        if node_is_folder {
            match display_folder(appplayer, number_selected, element, element_name, ui) {
                Err(e) => {
                    error!("error in displaying folder {}", e);
                }
                Ok(returned_value) => {
                    if returned_value {
                        file_selected = true;
                    }
                }
            }
        } else {
            // file and not a folder
            if display_file(appplayer, number_selected, element, element_name, ui) {
                file_selected = true;
            }
        }
    }
    Ok(file_selected)
}

/// Render the file tree side panel
fn render_file_tree_panel(app: &mut VirtualBookApp, ui: &mut Ui) {
    egui::SidePanel::left("tree_panel")
        .resizable(true)
        .default_width(200.0)
        .width_range(80.0..=500.0)
        .frame(Frame {
            inner_margin: Margin::symmetric(5.0, 0.0),
            outer_margin: Margin::ZERO,
            rounding: Rounding::ZERO,
            shadow: Shadow::NONE,
            fill: Color32::TRANSPARENT,
            ..Default::default()
        })
        .show_inside(ui, |ui| {
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
                                let current_view = match app.current_typed_no.is_empty() {
                                    true => &mut filestore.default_view,
                                    false => &mut filestore.filtered_view,
                                };

                                if let Some(view) = current_view {
                                    match display_tree(
                                        &mut app.appplayer,
                                        &mut app.current_typed_no,
                                        &mut view.root,
                                        ui,
                                    ) {
                                        Err(e) => {
                                            error!("error in display tree: {}", e);
                                        }
                                        Ok(_returned_value) => {}
                                    }
                                } else {
                                    ui.label(&app.i18n.aucun_fichiers);
                                }
                            }
                        });
                    });
            });
        });
}

/// Render the virtual book preview component
fn render_book_preview(app: &mut VirtualBookApp, ctx: &egui::Context, ui: &mut Ui) {
    let foffset: f64 = app.pid_regulated_offset_ms;

    // display virtualbook
    let c = VirtualBookComponent::from_some_indexedvirtualbook(
        app.appplayer.virtual_book.read().clone(),
    )
    .offset_ms(foffset)
    .xscale(app.xscale)
    .set_background_texture_id(app.background_textureid)
    .hide_scrollbar();

    if app.background_texture_handle.is_none() {
        app.background_texture_handle = Some(ctx.load_texture(
            "bgbookimage",
            app.background_texture_image.clone(),
            TextureOptions {
                magnification: TextureFilter::Nearest,
                minification: TextureFilter::Linear,
                wrap_mode: TextureWrapMode::Repeat,
            },
        ));
        app.background_textureid = Some(TextureId::from(
            app.background_texture_handle.as_ref().unwrap(),
        ));
    }

    let response_ui_component = c
        .set_background_texture_id(app.background_textureid)
        .ui_content(ui);

    if response_ui_component.clicked() {
        app.screen = Screen::Display;
    }
    response_ui_component.on_hover_text_at_pointer(&app.i18n.hover_click_to_enlarge_view);
}

#[cfg_attr(any(feature = "profiling"), profiling::function)]
pub(crate) fn ui_content(app: &mut VirtualBookApp, ctx: &egui::Context, ui: &mut Ui) {
    render_file_tree_panel(app, ui);

    egui::CentralPanel::default()
        .frame(Frame {
            inner_margin: Margin::ZERO,
            outer_margin: Margin::ZERO,
            rounding: Rounding::ZERO,
            shadow: Shadow::NONE,
            fill: Color32::TRANSPARENT,
            ..Default::default()
        })
        .show_inside(ui, |ui| {
            StripBuilder::new(ui)
                .size(Size::initial(100.0))
                .size(Size::remainder())
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        render_book_preview(app, ctx, ui);
                    });
                    strip.cell(|ui| {
                        // render playlist panel
                        ui_playlist_right_panel(app, ctx, ui);
                    });
                });
        });
}

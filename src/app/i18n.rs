#![allow(dead_code)]

pub struct I18NMessages {
    pub play: String,
    pub next: String,
    pub open_folder: String,
    pub reload_folder: String,
    pub quit: String,
    pub display: String,
    pub preferences: String,
    pub time_between_file: String,
    pub zoom: String,
    pub file: String,
    pub save_playlist: String,
    pub enter: String,
    pub aucun_fichiers: String,
    pub hide_num_pad: String,
    pub dark_light: String,
    pub filter: String,
    pub remove_file_from_list: String,
    pub button_remove: String,
    pub go_to_next_file: String,
}

fn _create_i18n_message_with_lang(language: Option<String>) -> Box<I18NMessages> {
    if let Some(lang) = language {
        match lang.as_str() {
            "fr" => create_i18n_fr_message(),
            _ => create_i18n_message(),
        }
    } else {
        // use default language
        create_i18n_message()
    }
}

pub fn create_i18n_message_with_lang(language: Option<String>) -> Box<I18NMessages> {
    log::debug!("command line language : {:?}", language);
    if let Some(_lang) = language.clone() {
        return _create_i18n_message_with_lang(language);
    }

    log::debug!("using LANG variable to detect language");

    let key = "LANG";

    use std::env;
    if let Some(lang_environement) = env::var_os(key) {
        let string_lang = lang_environement.to_string_lossy();
        if string_lang.len() >= 2 {
            return _create_i18n_message_with_lang(Some(string_lang[0..2].into()));
        }
    }

    use sys_locale::get_locale;

    let locale = get_locale().unwrap_or_else(|| String::from("en-US"));
    log::debug!("The current locale is {}", locale);
    if locale.len() >= 2 {
        _create_i18n_message_with_lang(Some(locale[0..2].into()))
    } else {
        create_i18n_message()
    }
}

pub fn create_i18n_message() -> Box<I18NMessages> {
    log::debug!("use english language");
    Box::new(I18NMessages {
        play: "Toggle Play Mode".into(),
        next: "Next".into(),
        open_folder: "Open Folder ...".into(),
        reload_folder: "Reload Folder".into(),
        quit: "Quit".into(),
        display: "Display".into(),
        preferences: "Preferences".into(),
        zoom: "Zoom".into(),
        file: "File".into(),
        save_playlist: "Save playlist ..".into(),
        enter: "Enter".into(),
        aucun_fichiers: "No_files".into(),
        hide_num_pad: "Hide numpad".into(),
        dark_light: "Light mode".into(),
        filter: "Filter".into(),
        remove_file_from_list: "Remove file from list".into(),
        button_remove: "Remove".into(),
        go_to_next_file: "Go to next file".into(),
        time_between_file: "Additional Time at the beginning (s):".into(),
    })
}

pub fn create_i18n_fr_message() -> Box<I18NMessages> {
    log::debug!("use french language");
    Box::new(I18NMessages {
        play: "Basculer le mode de jeu".into(),
        next: "Suivant".into(),
        open_folder: "Ouvrir un nouveau répertoire ...".into(),
        reload_folder: "Relire le contenu du répertoire".into(),
        quit: "Quitter".into(),
        display: "Affichage".into(),
        preferences: "Preferences".into(),
        time_between_file: "Temps supplementaire au debut du morceau (s):".into(),

        zoom: "Zoom :".into(),
        file: "Fichier".into(),
        save_playlist: "Enregistrer la liste ..".into(),
        enter: "Entrer".into(),
        aucun_fichiers: "Aucuns fichiers".into(),
        hide_num_pad: "Cacher le pavé numérique".into(),
        dark_light: "Couleures Claires".into(),
        filter: "Recherche".into(),
        button_remove: "Enlever".into(),
        remove_file_from_list: "Enlever le fichier de la liste".into(),
        go_to_next_file: "Lire le fichier suivant".into(),
    })
}

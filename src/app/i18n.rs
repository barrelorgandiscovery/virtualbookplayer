#![allow(dead_code)]

pub struct I18NMessages {
    pub play: String,
    pub next: String,
    pub open_folder: String,
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
}

pub fn create_i18n_message_with_lang(lang: String) -> Box<I18NMessages> {
    match lang.as_str() {
        "fr" => create_i18n_fr_message(),
        _ => create_i18n_message(),
    }
}

pub fn create_i18n_message() -> Box<I18NMessages> {
    Box::new(I18NMessages {
        play: "Play".into(),
        next: "Next".into(),
        open_folder: "Open Folder ...".into(),
        quit: "Quit".into(),
        display: "Display".into(),
        preferences: "Preferences".into(),
        time_between_file: "Temps supplementaire au debut du morceau :".into(),
        zoom: "Zoom".into(),
        file: "File".into(),
        save_playlist: "Save playlist ..".into(),
        enter: "Enter".into(),
        aucun_fichiers: "No_files".into(),
        hide_num_pad: "Hide numpad".into(),
        dark_light: "Light mode".into(),
    })
}

pub fn create_i18n_fr_message() -> Box<I18NMessages> {
    Box::new(I18NMessages {
        play: "Jouer".into(),
        next: "Suivant".into(),
        open_folder: "Ouvrir un nouveau répertoire ...".into(),
        quit: "Quitter".into(),
        display: "Affichage".into(),
        preferences: "Preferences".into(),
        time_between_file: "Additional Time at the beginning :".into(),
        zoom: "Zoom :".into(),
        file: "Fichier".into(),
        save_playlist: "Enregistrer la liste ..".into(),
        enter: "Entrer".into(),
        aucun_fichiers: "Aucuns fichiers".into(),
        hide_num_pad: "Cacher le pavé numérique".into(),
        dark_light: "Couleures Claires".into(),
    })
}

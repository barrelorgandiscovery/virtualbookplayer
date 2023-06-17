pub struct I18NMessages {
    pub play: String,
    pub next: String,
    pub open_folder: String,
    pub quit: String,
    pub display: String,
    pub zoom: String,
    pub file: String,
    pub save_playlist: String,
    pub enter: String,
    pub aucun_fichiers: String,
}

pub fn create_i18n_message() -> Box<I18NMessages> {
    Box::new(I18NMessages {
        play: "Play".into(),
        next: "Next".into(),
        open_folder: "Open Folder ...".into(),
        quit: "Quit".into(),
        display: "Display".into(),
        zoom: "Zoom".into(),
        file: "File".into(),
        save_playlist: "Save playlist ..".into(),
        enter: "Enter".into(),
        aucun_fichiers: "no_files".into(),
    })
}

pub fn create_i18n_fr_message() -> Box<I18NMessages> {
    Box::new(I18NMessages {
        play: "Jouer".into(),
        next: "Suivant".into(),
        open_folder: "Ouvrir un nouveau répertoire ...".into(),
        quit: "Quitter".into(),
        display: "Affichage".into(),
        zoom: "Zoom :".into(),
        file: "Fichier".into(),
        save_playlist: "Enregistrer la liste ..".into(),
        enter: "Entrer".into(),
        aucun_fichiers: "Aucuns fichiers".into(),
    })
}

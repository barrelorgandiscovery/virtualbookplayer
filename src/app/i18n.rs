#![allow(dead_code)]

use crate::gfx::BookEffectKind;

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
    pub catalog: String,
    pub history: String,
    pub recent_played: String,
    pub ranking: String,
    pub sort_stars: String,
    pub sort_plays: String,
    pub sort_both: String,
    pub add_to_playlist: String,
    pub file_missing: String,
    pub history_empty: String,
    pub history_needs_folder: String,
    pub history_loading: String,
    pub last_played: String,
    pub rel_seconds: String,
    pub rel_minute: String,
    pub rel_minutes: String,
    pub rel_hour: String,
    pub rel_hours: String,
    pub rel_yesterday: String,
    pub rel_days: String,
    pub rel_month: String,
    pub rel_months: String,
    pub rel_year: String,
    pub rel_years: String,
    pub remove_file_from_list: String,
    pub button_remove: String,
    pub go_to_next_file: String,
    pub lattence_jeu: String,
    pub book_effect: String,
    pub book_effect_none: String,
    pub book_effect_note_fire: String,
    pub book_effect_sparkle: String,
    pub book_effect_ripple: String,
    pub book_effect_neon_lane: String,
    pub book_effect_arcade_burst: String,
    pub book_effect_scale: String,
    pub switch_auto: String,

    pub hover_click_to_enlarge_view: String,
    pub hover_activate_the_play_of_the_playlist: String,
    pub hover_pause_the_tune: String,
    pub hover_resume_the_tune: String,
    pub hover_retour_a_la_playlist: String,
    pub play_count_tooltip: String,
    pub folder_read_tooltip: String,
    pub star_file_tooltip: String,
    pub star_count_tooltip: String,
    pub panel_title: String,
}

impl I18NMessages {
    pub fn book_effect_label(&self, kind: BookEffectKind) -> &str {
        match kind {
            BookEffectKind::None => self.book_effect_none.as_str(),
            BookEffectKind::NoteFire => self.book_effect_note_fire.as_str(),
            BookEffectKind::Sparkle => self.book_effect_sparkle.as_str(),
            BookEffectKind::Ripple => self.book_effect_ripple.as_str(),
            BookEffectKind::NeonLane => self.book_effect_neon_lane.as_str(),
            BookEffectKind::ArcadeBurst => self.book_effect_arcade_burst.as_str(),
        }
    }
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
        catalog: "Catalog".into(),
        history: "History".into(),
        recent_played: "Recent".into(),
        ranking: "Ranking".into(),
        sort_stars: "Stars".into(),
        sort_plays: "Most played".into(),
        sort_both: "Both".into(),
        add_to_playlist: "Add to playlist".into(),
        file_missing: "This file is no longer in the folder".into(),
        history_empty: "No tunes played yet".into(),
        history_needs_folder: "Open a folder to see play history".into(),
        history_loading: "Loading…".into(),
        last_played: "Last played".into(),
        rel_seconds: "a few seconds ago".into(),
        rel_minute: "a minute ago".into(),
        rel_minutes: "{n} minutes ago".into(),
        rel_hour: "an hour ago".into(),
        rel_hours: "{n} hours ago".into(),
        rel_yesterday: "yesterday".into(),
        rel_days: "{n} days ago".into(),
        rel_month: "a month ago".into(),
        rel_months: "{n} months ago".into(),
        rel_year: "a year ago".into(),
        rel_years: "{n} years ago".into(),
        remove_file_from_list: "Remove file from list".into(),
        button_remove: "Remove".into(),
        go_to_next_file: "Go to next file".into(),
        time_between_file: "Additional Time at the beginning (s):".into(),
        lattence_jeu: "Play lattency (ms):".into(),
        book_effect: "Book visual effect:".into(),
        book_effect_none: "No effect".into(),
        book_effect_note_fire: "Note fire".into(),
        book_effect_sparkle: "Sparkle".into(),
        book_effect_ripple: "Ripple".into(),
        book_effect_neon_lane: "Neon lane".into(),
        book_effect_arcade_burst: "Arcade burst".into(),
        book_effect_scale: "Effect intensity:".into(),
        switch_auto: "Automatic Switch".into(),

        hover_click_to_enlarge_view: "Click to enlarge the view".into(),
        hover_activate_the_play_of_the_playlist: "Activate the play of the playlist".into(),
        hover_pause_the_tune: "Pause the tune".into(),
        hover_resume_the_tune: "Resume the tune".into(),
        hover_retour_a_la_playlist: "Back to playlist".into(),
        play_count_tooltip: "Number of times this file has been played".into(),
        folder_read_tooltip: "Played files in this folder and its subfolders".into(),
        star_file_tooltip: "Star this file".into(),
        star_count_tooltip: "Number of times this file has been starred".into(),
        panel_title: "playlist".into(),
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
        catalog: "Catalogue".into(),
        history: "Historique".into(),
        recent_played: "Récents".into(),
        ranking: "Classement".into(),
        sort_stars: "Étoiles".into(),
        sort_plays: "Plus joués".into(),
        sort_both: "Les deux".into(),
        add_to_playlist: "Ajouter à la liste".into(),
        file_missing: "Ce fichier n'est plus dans le dossier".into(),
        history_empty: "Aucun morceau joué pour l'instant".into(),
        history_needs_folder: "Ouvrez un dossier pour voir l'historique".into(),
        history_loading: "Chargement…".into(),
        last_played: "Dernière lecture".into(),
        rel_seconds: "il y a quelques secondes".into(),
        rel_minute: "il y a une minute".into(),
        rel_minutes: "il y a {n} minutes".into(),
        rel_hour: "il y a une heure".into(),
        rel_hours: "il y a {n} heures".into(),
        rel_yesterday: "hier".into(),
        rel_days: "il y a {n} jours".into(),
        rel_month: "il y a un mois".into(),
        rel_months: "il y a {n} mois".into(),
        rel_year: "il y a un an".into(),
        rel_years: "il y a {n} ans".into(),
        button_remove: "Enlever".into(),
        remove_file_from_list: "Enlever le fichier de la liste".into(),
        go_to_next_file: "Lire le fichier suivant".into(),
        lattence_jeu: "Lattence (ms):".into(),
        book_effect: "Effet visuel du carton :".into(),
        book_effect_none: "Aucun effet".into(),
        book_effect_note_fire: "Feu de notes".into(),
        book_effect_sparkle: "Étincelles".into(),
        book_effect_ripple: "Ondulations".into(),
        book_effect_neon_lane: "Voie néon".into(),
        book_effect_arcade_burst: "Explosion arcade".into(),
        book_effect_scale: "Intensité de l'effet :".into(),
        switch_auto: "Bascule automatique".into(),

        hover_click_to_enlarge_view: "Clickez pour agrandir".into(),
        hover_activate_the_play_of_the_playlist: "Jouer ou Arreter le jeu de la playlist".into(),
        hover_pause_the_tune: "Mettre le morceau en pause".into(),
        hover_resume_the_tune: "Reprendre le morceau".into(),
        hover_retour_a_la_playlist: "Retour à la playlist".into(),
        play_count_tooltip: "Nombre de fois que ce fichier a été joué".into(),
        folder_read_tooltip: "Fichiers lus dans ce dossier et ses sous-dossiers".into(),
        star_file_tooltip: "Mettre une étoile à ce fichier".into(),
        star_count_tooltip: "Nombre de fois que ce fichier a été étoilé".into(),
        panel_title: "Liste de lecture".into(),
    })
}

#![allow(dead_code)]

use crate::gfx::BookEffectKind;

use super::theme::ThemeId;

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
    pub theme: String,
    pub theme_night: String,
    pub theme_console: String,
    pub theme_marquee: String,
    pub theme_parchment: String,
    pub theme_butter: String,
    pub theme_loft: String,
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
    pub fn theme_name(&self, id: ThemeId) -> &str {
        match id {
            ThemeId::Night => self.theme_night.as_str(),
            ThemeId::Console => self.theme_console.as_str(),
            ThemeId::Marquee => self.theme_marquee.as_str(),
            ThemeId::Parchment => self.theme_parchment.as_str(),
            ThemeId::Butter => self.theme_butter.as_str(),
            ThemeId::Loft => self.theme_loft.as_str(),
        }
    }

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
        play: "Play".into(),
        next: "Next".into(),
        open_folder: "Open folder…".into(),
        reload_folder: "Reload folder".into(),
        quit: "Quit".into(),
        display: "Display".into(),
        preferences: "Preferences".into(),
        zoom: "Zoom".into(),
        file: "File".into(),
        save_playlist: "Save playlist…".into(),
        enter: "Enter".into(),
        aucun_fichiers: "No files".into(),
        hide_num_pad: "Hide numpad".into(),
        theme: "Theme".into(),
        theme_night: "Night".into(),
        theme_console: "Console".into(),
        theme_marquee: "Marquee".into(),
        theme_parchment: "Parchment".into(),
        theme_butter: "Butter".into(),
        theme_loft: "Loft".into(),
        filter: "Search".into(),
        catalog: "Catalog".into(),
        history: "History".into(),
        recent_played: "Recently played".into(),
        ranking: "Ranking".into(),
        sort_stars: "Stars".into(),
        sort_plays: "Most played".into(),
        sort_both: "Stars, then plays".into(),
        add_to_playlist: "Add to playlist".into(),
        file_missing: "This file is no longer in the folder".into(),
        history_empty: "Nothing played yet".into(),
        history_needs_folder: "Open a folder to see the history".into(),
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
        remove_file_from_list: "Remove from the playlist".into(),
        button_remove: "Remove".into(),
        go_to_next_file: "Next file".into(),
        time_between_file: "Extra time at the start (s):".into(),
        lattence_jeu: "Playback latency (ms):".into(),
        book_effect: "Book visual effect:".into(),
        book_effect_none: "No effect".into(),
        book_effect_note_fire: "Note fire".into(),
        book_effect_sparkle: "Sparkle".into(),
        book_effect_ripple: "Ripple".into(),
        book_effect_neon_lane: "Neon lane".into(),
        book_effect_arcade_burst: "Arcade burst".into(),
        book_effect_scale: "Effect intensity:".into(),
        switch_auto: "Return to the roll view".into(),

        hover_click_to_enlarge_view: "Click to enlarge".into(),
        hover_activate_the_play_of_the_playlist: "Play or stop the playlist".into(),
        hover_pause_the_tune: "Pause".into(),
        hover_resume_the_tune: "Resume".into(),
        hover_retour_a_la_playlist: "Back to the playlist".into(),
        play_count_tooltip: "Plays".into(),
        folder_read_tooltip: "Played files in this folder and below".into(),
        star_file_tooltip: "Add a star".into(),
        star_count_tooltip: "Stars".into(),
        panel_title: "Playlist".into(),
    })
}

pub fn create_i18n_fr_message() -> Box<I18NMessages> {
    log::debug!("use french language");
    Box::new(I18NMessages {
        play: "Lecture".into(),
        next: "Suivant".into(),
        open_folder: "Ouvrir un répertoire…".into(),
        reload_folder: "Recharger le répertoire".into(),
        quit: "Quitter".into(),
        display: "Affichage".into(),
        preferences: "Préférences".into(),
        time_between_file: "Temps supplémentaire au début (s) :".into(),

        zoom: "Zoom :".into(),
        file: "Fichier".into(),
        save_playlist: "Enregistrer la liste…".into(),
        enter: "Valider".into(),
        aucun_fichiers: "Aucun fichier".into(),
        hide_num_pad: "Masquer le pavé numérique".into(),
        theme: "Thème".into(),
        theme_night: "Nuit".into(),
        theme_console: "Console".into(),
        theme_marquee: "Chapiteau".into(),
        theme_parchment: "Carton".into(),
        theme_butter: "Beurre".into(),
        theme_loft: "Atelier".into(),
        filter: "Recherche".into(),
        catalog: "Catalogue".into(),
        history: "Historique".into(),
        recent_played: "Récemment joués".into(),
        ranking: "Classement".into(),
        sort_stars: "Étoiles".into(),
        sort_plays: "Les plus joués".into(),
        sort_both: "Étoiles, puis lectures".into(),
        add_to_playlist: "Ajouter à la liste".into(),
        file_missing: "Ce fichier n'est plus dans le répertoire".into(),
        history_empty: "Aucune lecture pour l'instant".into(),
        history_needs_folder: "Ouvrez un répertoire pour voir l'historique".into(),
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
        button_remove: "Retirer".into(),
        remove_file_from_list: "Retirer de la liste".into(),
        go_to_next_file: "Fichier suivant".into(),
        lattence_jeu: "Latence de lecture (ms) :".into(),
        book_effect: "Effet visuel du carton :".into(),
        book_effect_none: "Aucun effet".into(),
        book_effect_note_fire: "Feu de notes".into(),
        book_effect_sparkle: "Étincelles".into(),
        book_effect_ripple: "Ondulations".into(),
        book_effect_neon_lane: "Voie néon".into(),
        book_effect_arcade_burst: "Explosion arcade".into(),
        book_effect_scale: "Intensité de l'effet :".into(),
        switch_auto: "Retour automatique à l'affichage".into(),

        hover_click_to_enlarge_view: "Cliquer pour agrandir".into(),
        hover_activate_the_play_of_the_playlist: "Lire ou arrêter la liste".into(),
        hover_pause_the_tune: "Pause".into(),
        hover_resume_the_tune: "Reprendre".into(),
        hover_retour_a_la_playlist: "Retour à la liste".into(),
        play_count_tooltip: "Lectures".into(),
        folder_read_tooltip: "Fichiers lus ici et dans les sous-répertoires".into(),
        star_file_tooltip: "Ajouter une étoile".into(),
        star_count_tooltip: "Étoiles".into(),
        panel_title: "Liste de lecture".into(),
    })
}

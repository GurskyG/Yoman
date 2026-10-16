use std::path::Path;

use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use crate::entry::{self, OpenedEntry};
use crate::journal::{self, JournalSnapshot};
use crate::settings::{self, Settings};
use crate::write::{self, SaveOutcome, SaveResponse};

#[tauri::command]
pub fn journal_status(app: tauri::AppHandle) -> JournalSnapshot {
    match saved_folder(&app) {
        Ok(None) => journal::unconfigured_snapshot(),
        Ok(Some(folder)) => journal::snapshot_for_folder(Path::new(&folder)),
        Err(message) => journal::error_snapshot(message),
    }
}

#[tauri::command]
pub async fn choose_journal_folder(app: tauri::AppHandle) -> Result<JournalSnapshot, String> {
    let dialog_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut builder = dialog_app
            .dialog()
            .file()
            .set_title("Choose journal folder");
        if let Some(window) = dialog_app.get_webview_window("main") {
            builder = builder.set_parent(&window);
        }
        builder.blocking_pick_folder()
    })
    .await
    .map_err(|_| {
        "Yoman could not open the folder picker. Try again. No files were changed.".to_string()
    })?;

    let Some(picked) = picked else {
        return Ok(journal_status(app));
    };

    let path = picked.into_path().map_err(|_| {
        "Yoman could not read the selected folder path. Try again. No files were changed."
            .to_string()
    })?;

    journal::list_markdown_files(&path).map_err(|err| {
        format!(
            "{} Yoman kept the previous journal folder and did not modify any files.",
            err.user_message()
        )
    })?;

    let canonical = path.canonicalize().map_err(|err| {
        format!(
            "Yoman could not resolve the selected folder ({err}). The previous journal folder was left unchanged."
        )
    })?;
    let folder = canonical.to_str().ok_or_else(|| {
        "The selected folder path is not valid Unicode, so Yoman cannot store it. Choose a different folder. No files were changed.".to_string()
    })?;

    let settings_path = settings::settings_path(&app)?;
    settings::save_settings(
        &settings_path,
        &Settings {
            journal_folder: Some(folder.to_string()),
        },
    )?;

    Ok(journal::snapshot_for_folder(&canonical))
}

#[tauri::command]
pub fn read_entry(app: tauri::AppHandle, file_name: String) -> Result<OpenedEntry, String> {
    let folder = saved_folder(&app)?.ok_or_else(|| {
        "Choose a journal folder before opening an entry. No files were changed.".to_string()
    })?;
    entry::read_entry(Path::new(&folder), &file_name).map_err(|err| err.user_message())
}

#[tauri::command]
pub fn create_entry(app: tauri::AppHandle) -> Result<SaveResponse, String> {
    let folder = journal_folder(&app)?;
    let (now, date, stamp) = write::current_clock();
    let entry = write::create_entry(Path::new(&folder), now, &date, &stamp)
        .map_err(|err| err.user_message())?;
    Ok(write::response_saved(
        entry,
        Some(journal::snapshot_for_folder(Path::new(&folder))),
    ))
}

#[tauri::command]
pub fn save_entry(
    app: tauri::AppHandle,
    file_name: String,
    title: String,
    body: String,
    base_original: String,
    mode: String,
    tags: Vec<String>,
) -> Result<SaveResponse, String> {
    let folder = journal_folder(&app)?;
    let (now, date, stamp) = write::current_clock();
    let path = Path::new(&folder);
    let journal = journal::snapshot_for_folder(path);
    match mode.as_str() {
        "update" => match write::save_entry(
            path,
            &file_name,
            &title,
            &body,
            &base_original,
            &stamp,
            &tags,
        )
        .map_err(|err| err.user_message())?
        {
            SaveOutcome::Saved { entry } => Ok(write::response_saved(entry, Some(journal))),
            SaveOutcome::Conflict { disk } => Ok(write::response_conflict(disk)),
        },
        "overwrite" => {
            let entry = write::overwrite_entry(
                path,
                &file_name,
                &title,
                &body,
                &base_original,
                &stamp,
                &tags,
            )
            .map_err(|err| err.user_message())?;
            Ok(write::response_saved(entry, Some(journal)))
        }
        "copy" => {
            let entry = write::save_copy(
                path,
                &title,
                &body,
                &base_original,
                now,
                &date,
                &stamp,
                &tags,
            )
            .map_err(|err| err.user_message())?;
            Ok(write::response_saved(entry, Some(journal)))
        }
        _ => Err(
            "Yoman did not recognize that save action. Nothing was written, and the editor text is still here."
                .to_string(),
        ),
    }
}

#[tauri::command]
pub fn delete_entry(app: tauri::AppHandle, file_name: String) -> Result<JournalSnapshot, String> {
    let folder = journal_folder(&app)?;
    write::delete_entry(Path::new(&folder), &file_name).map_err(|err| match err {
        write::WriteError::Io { detail } => format!(
            "Yoman could not move the file to the trash ({detail}). The entry is still in the folder."
        ),
        other => other.user_message(),
    })?;
    Ok(journal::snapshot_for_folder(Path::new(&folder)))
}

fn journal_folder(app: &tauri::AppHandle) -> Result<String, String> {
    saved_folder(app)?.ok_or_else(|| {
        "Choose a journal folder before changing entries. No files were written.".to_string()
    })
}

fn saved_folder(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    let path = settings::settings_path(app)?;
    let settings = settings::load_settings(&path)?;
    Ok(settings.journal_folder.filter(|folder| !folder.is_empty()))
}

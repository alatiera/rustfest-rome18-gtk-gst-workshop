use gtk::prelude::*;
use gst::prelude::*;
use gio::prelude::*;

use utils;
use headerbar;
use settings::{RecordFormat, SnapshotFormat};

use std::rc::{Rc, Weak};
use std::cell::RefCell;
use std::fs::create_dir_all;

use gst;

// Our refcounted application struct for containing all the
// state we have to carry around
#[derive(Clone)]
pub struct App(pub Rc<RefCell<AppInner>>);

pub struct AppWeak(pub Weak<RefCell<AppInner>>);

impl AppWeak {
    pub fn upgrade(&self) -> Option<App> {
        self.0.upgrade().map(App)
    }
}

pub struct AppInner {
    pub main_window: Option<gtk::ApplicationWindow>,
    pub pipeline: Option<gst::Pipeline>,

    // Snapshot timer state
    pub timeout: Option<glib::source::SourceId>,
    pub remaining_secs_before_snapshot: u32,
}

// Here we specify our custom, application specific CSS styles for various widgets
const STYLE: &'static str = "
#countdown-label {
    background-color: rgba(192, 192, 192, 0.8);
    color: black;
    font-size: 42pt;
    font-weight: bold;
}";

// Construct the settings dialog and ensure that the settings file exists and is loaded
pub fn build_settings_window(parent: &Option<gtk::Window>) {
    let s = utils::get_settings_file_path();

    if !s.exists() {
        if let Some(parent_dir) = s.parent() {
            if !parent_dir.exists() {
                if let Err(e) = create_dir_all(parent_dir) {
                    utils::show_error_dialog(
                        parent.as_ref(),
                        false,
                        format!(
                            "Error when trying to build settings snapshot_directory '{}': {:?}",
                            parent_dir.display(),
                            e
                        )
                        .as_str(),
                    );
                }
            }
        }
    }

    let settings = utils::load_settings();

    //
    // BUILDING UI
    //
    let dialog = gtk::Dialog::new_with_buttons(
        Some("Snapshot settings"),
        parent.as_ref(),
        gtk::DialogFlags::MODAL,
        &[("Close", gtk::ResponseType::Close.into())],
    );

    let grid = gtk::Grid::new();
    grid.set_column_spacing(4);
    grid.set_row_spacing(4);
    grid.set_margin_bottom(10);

    //
    // SNAPSHOT FOLDER
    //
    let snapshot_directory_label = gtk::Label::new("Snapshot directory");
    let snapshot_directory_chooser_but = gtk::FileChooserButton::new(
        "Pick a directory to save snapshots",
        gtk::FileChooserAction::SelectFolder,
    );

    snapshot_directory_label.set_halign(gtk::Align::Start);
    snapshot_directory_chooser_but.set_filename(settings.snapshot_directory);

    grid.attach(&snapshot_directory_label, 0, 0, 1, 1);
    grid.attach(&snapshot_directory_chooser_but, 1, 0, 3, 1);

    //
    // SNAPSHOT FORMAT OPTIONS
    //
    let format_label = gtk::Label::new("Snapshot format");
    let snapshot_format = gtk::ComboBoxText::new();

    format_label.set_halign(gtk::Align::Start);

    snapshot_format.append_text("JPEG");
    snapshot_format.append_text("PNG");
    snapshot_format.set_active(match settings.snapshot_format {
        SnapshotFormat::JPEG => 0,
        SnapshotFormat::PNG => 1,
    });
    snapshot_format.set_hexpand(true);

    grid.attach(&format_label, 0, 1, 1, 1);
    grid.attach(&snapshot_format, 1, 1, 3, 1);

    //
    // TIMER LENGTH
    //
    let timer_label = gtk::Label::new("Timer length (in seconds)");
    let timer_entry = gtk::SpinButton::new_with_range(0., 15., 1.);

    timer_label.set_halign(gtk::Align::Start);
    timer_label.set_hexpand(true);

    timer_entry.set_value(settings.timer_length as f64);

    grid.attach(&timer_label, 0, 2, 1, 1);
    grid.attach(&timer_entry, 1, 2, 3, 1);

    //
    // RECORD FOLDER
    //
    let record_directory_label = gtk::Label::new("Record directory");
    let record_directory_chooser_but = gtk::FileChooserButton::new(
        "Pick a directory to save records",
        gtk::FileChooserAction::SelectFolder,
    );

    record_directory_label.set_halign(gtk::Align::Start);
    record_directory_chooser_but.set_filename(settings.record_directory);

    grid.attach(&record_directory_label, 0, 3, 1, 1);
    grid.attach(&record_directory_chooser_but, 1, 3, 3, 1);

    //
    // RECORD FORMAT OPTIONS
    //
    let format_label = gtk::Label::new("Record format");
    let record_format = gtk::ComboBoxText::new();

    format_label.set_halign(gtk::Align::Start);

    record_format.append_text("H264/MP4");
    record_format.append_text("VP8/WebM");
    record_format.set_active(match settings.record_format {
        RecordFormat::H264Mp4 => 0,
        RecordFormat::Vp8WebM => 1,
    });
    record_format.set_hexpand(true);

    grid.attach(&format_label, 0, 4, 1, 1);
    grid.attach(&record_format, 1, 4, 3, 1);

    //
    // PUTTING WIDGETS INTO DIALOG
    //
    let content_area = dialog.get_content_area();
    content_area.pack_start(&grid, true, true, 0);
    content_area.set_border_width(10);

    //
    // ADDING SETTINGS "AUTOMATIC" SAVE
    //
    save_settings!(timer_entry, connect_value_changed,
                   snapshot_directory_chooser_but, snapshot_format, record_directory_chooser_but, record_format =>
                   move |timer_entry| {
        utils::save_settings(&snapshot_directory_chooser_but, &snapshot_format, &timer_entry,
                             &record_directory_chooser_but, &record_format);
    });

    save_settings!(snapshot_format, connect_changed,
                   snapshot_directory_chooser_but, timer_entry, record_directory_chooser_but, record_format =>
                   move |snapshot_format| {
        utils::save_settings(&snapshot_directory_chooser_but, &snapshot_format, &timer_entry,
                             &record_directory_chooser_but, &record_format);
    });

    save_settings!(snapshot_directory_chooser_but, connect_file_set, timer_entry, snapshot_format,
                   record_directory_chooser_but, record_format =>
                   move |snapshot_directory_chooser_but| {
        utils::save_settings(&snapshot_directory_chooser_but, &snapshot_format, &timer_entry,
                             &record_directory_chooser_but, &record_format);
    });

    save_settings!(record_format, connect_changed,
                   snapshot_directory_chooser_but, timer_entry, record_directory_chooser_but, snapshot_format =>
                   move |record_format| {
        utils::save_settings(&snapshot_directory_chooser_but, &snapshot_format, &timer_entry,
                             &record_directory_chooser_but, &record_format);
    });

    save_settings!(record_directory_chooser_but, connect_file_set,
                   timer_entry, snapshot_format, snapshot_directory_chooser_but, record_format =>
                   move |record_directory_chooser_but| {
        utils::save_settings(&snapshot_directory_chooser_but, &snapshot_format, &timer_entry,
                             &record_directory_chooser_but, &record_format);
    });

    dialog.connect_response(|dialog, _| {
        dialog.destroy();
    });

    dialog.set_resizable(false);
    dialog.show_all();
}

impl App {
    pub fn new() -> App {
        App(Rc::new(RefCell::new(AppInner {
            main_window: None,
            pipeline: None,
            timeout: None,
            remaining_secs_before_snapshot: 0,
        })))
    }

    pub fn downgrade(&self) -> AppWeak {
        AppWeak(Rc::downgrade(&self.0))
    }

    pub fn on_startup(&self, application: &gtk::Application) {
        // Load our custom CSS style-sheet and set it as the application
        // specific style-sheet for this whole application
        let provider = gtk::CssProvider::new();
        provider
            .load_from_data(STYLE.as_bytes())
            .expect("Failed to load CSS");
        gtk::StyleContext::add_provider_for_screen(
            &gdk::Screen::get_default().expect("Error initializing gtk css provider."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // Create our UI actions
        self.connect_actions(application);

        // Build the UI but don't show it yet
        self.build_ui(application);
    }

    pub fn on_activate(&self) {
        let inner = self.0.borrow_mut();
        // We only show our window here once the application
        // is activated. This means that when a second instance
        // is started, the window of the first instance will be
        // brought to the foreground
        if let Some(ref main_window) = inner.main_window {
            main_window.show_all();
            main_window.present();
        }

        // Once the UI is shown, start the GStreamer pipeline. If
        // an error happens, we immediately shut down
        if let Some(ref pipeline) = inner.pipeline {
            if let Err(err) = pipeline.set_state(gst::State::Playing).into_result() {
                utils::show_error_dialog(
                    inner.main_window.as_ref(),
                    true,
                    format!("Failed to set pipeline to playing: {:?}", err).as_str(),
                );
            }
        }
    }

    pub fn on_shutdown(&self) {
        if let Some(ref pipeline) = self.0.borrow().pipeline {
            // This might fail but as we shut down right now anyway this
            // doesn't matter
            let _ = pipeline.set_state(gst::State::Null);
        }
    }

    fn connect_actions(&self, application: &gtk::Application) {
        // Create actions for our settings and about dialogs
        //
        // This can be activated from anywhere where we have access
        // to the application, not just the main window
        let settings = gio::SimpleAction::new("settings", None);

        // When activated, show a settings dialog
        let weak_application = application.downgrade();
        settings.connect_activate(move |_action, _parameter| {
            let application = upgrade_weak!(weak_application);

            build_settings_window(&application.get_active_window());
        });

        let about = gio::SimpleAction::new("about", None);

        // When activated, show an about dialog
        let weak_application = application.downgrade();
        about.connect_activate(move |_action, _parameter| {
            let application = upgrade_weak!(weak_application);

            let p = gtk::AboutDialog::new();

            p.set_authors(&["Sebastian Dröge", "Guillaume Gomez"]);
            p.set_website_label("github repository");
            p.set_website("https://github.com/sdroege/rustfest-rome18-gtk-gst-workshop");
            p.set_comments("A webcam viewer written with gtk-rs and gstreamer-rs");
            p.set_copyright("This is under MIT license");
            if let Some(window) = application.get_active_window() {
                p.set_transient_for(&window);
            }
            p.set_modal(true);
            p.set_program_name("RustFest 2018 GTK+ & GStreamer WebCam Viewer");

            // When any response on the dialog happens, we simply destroy it.
            //
            // We don't have any custom buttons added so this will only ever
            // handle the close button, otherwise we could distinguish the
            // buttons by the response
            p.connect_response(|dialog, _response| {
                dialog.destroy();
            });

            p.show_all();
        });

        application.add_action(&settings);
        application.add_action(&about);
    }

    // When the snapshot button is clicked, we have to start the timer, stop the timer or directly
    // snapshot
    fn on_snapshot_button_clicked(
        &self,
        snapshot_button: &gtk::ToggleButton,
        overlay_text: &gtk::Label,
    ) {
        let settings = utils::load_settings();
        let mut inner = self.0.borrow_mut();

        // If we're currently doing a countdown, cancel it
        if let Some(t) = inner.timeout.take() {
            glib::source::source_remove(t);
            overlay_text.set_visible(false);
            return;
        } else if settings.timer_length == 0 {
            // Otherwise take a snapshot immediately if there's
            // no timer length or start the timer
            //
            // Set the togglebutton unchecked again
            snapshot_button.set_state_flags(
                snapshot_button.get_state_flags() & !gtk::StateFlags::CHECKED,
                true,
            );

            // Make sure to drop the borrow before calling any other
            // app methods
            drop(inner);

            self.take_snapshot();
        } else {
            // Make the overlay visible, remember how much we have to count
            // down and start our timeout for the timer
            overlay_text.set_visible(true);
            overlay_text.set_text(&settings.timer_length.to_string());

            inner.remaining_secs_before_snapshot = settings.timer_length;

            let overlay_text_weak = overlay_text.downgrade();
            let snapshot_button_weak = snapshot_button.downgrade();
            let app_weak = self.downgrade();
            // The closure is called every 1000ms
            let source = gtk::timeout_add(1000, move || {
                let app = upgrade_weak!(app_weak, glib::Continue(false));
                let snapshot_button = upgrade_weak!(snapshot_button_weak, glib::Continue(false));
                let overlay_text = upgrade_weak!(overlay_text_weak, glib::Continue(false));

                let mut inner = app.0.borrow_mut();

                inner.remaining_secs_before_snapshot -= 1;
                if inner.remaining_secs_before_snapshot == 0 {
                    // Set the togglebutton unchecked again and make
                    // the overlay text invisible
                    overlay_text.set_visible(false);
                    snapshot_button.set_state_flags(
                        snapshot_button.get_state_flags() & !gtk::StateFlags::CHECKED,
                        true,
                    );
                    inner.timeout = None;
                } else {
                    overlay_text.set_text(&inner.remaining_secs_before_snapshot.to_string());
                }

                if inner.remaining_secs_before_snapshot == 0 {
                    // Make sure to drop the borrow before calling any other
                    // app methods
                    drop(inner);

                    app.take_snapshot();
                    glib::Continue(false)
                } else {
                    glib::Continue(true)
                }
            });

            inner.timeout = Some(source);
        }
    }

    // When the record button is clicked, we have to start or stop recording
    fn on_record_button_clicked(&self, record_button: &gtk::ToggleButton) {
        // Start/stop recording based on button active'ness
        if record_button.get_active() {
            self.start_recording(record_button);
        } else {
            self.stop_recording();
        }
    }

    fn build_ui(&self, application: &gtk::Application) {
        let window = gtk::ApplicationWindow::new(application);
        self.0.borrow_mut().main_window = Some(window.clone());

        window.set_title("RustFest 2018 GTK+ & GStreamer WebCam Viewer");
        window.set_border_width(5);
        window.set_position(gtk::WindowPosition::Center);
        window.set_default_size(350, 300);

        // Create headerbar for the application, including the main
        // menu and a close button
        let header_bar = headerbar::HeaderBar::default();
        // FIXME: these should not be needed
        let snapshot_button = &header_bar.snapshot;
        let record_button = &header_bar.record;

        // Pack the snapshot/record buttons on the left, the main menu on
        // the right of the header bar and set it on our window
        window.set_titlebar(&header_bar.container);

        // Create an overlay for showing the seconds until a snapshot
        // This is hidden while we're not doing a countdown
        let overlay = gtk::Overlay::new();

        let overlay_text = gtk::Label::new("0");
        // Our label should have the countdown-label style from the stylesheet
        gtk::WidgetExt::set_name(&overlay_text, "countdown-label");

        // Center the label in the overlay and give it a width of 3 characters
        // to always have the same width independent of the width of the current
        // number
        overlay_text.set_halign(gtk::Align::Center);
        overlay_text.set_valign(gtk::Align::Center);
        overlay_text.set_width_chars(3);
        overlay_text.set_no_show_all(true);
        overlay_text.set_visible(false);

        overlay.add_overlay(&overlay_text);

        // When the snapshot button is clicked we need to start the
        // countdown, stop the countdown or directly do a snapshot
        let app_weak = self.downgrade();
        snapshot_button.connect_clicked(move |snapshot_button| {
            let app = upgrade_weak!(app_weak);
            app.on_snapshot_button_clicked(&snapshot_button, &overlay_text);
        });

        // When the record button is clicked we need to start or stop
        // recording based on its state
        let app_weak = self.downgrade();
        record_button.connect_clicked(move |record_button| {
            let app = upgrade_weak!(app_weak);
            app.on_record_button_clicked(&record_button);
        });

        // Create the pipeline and if that fails, shut down and
        // remember the error that happened
        let (pipeline, view) = match self.create_pipeline() {
            Err(err) => {
                utils::show_error_dialog(
                    Some(&window),
                    true,
                    format!("Error creating pipeline: {:?}", err).as_str(),
                );
                return;
            }
            Ok(res) => res,
        };

        // Store the pipeline for later usage and add the view widget
        // to the UI
        self.0.borrow_mut().pipeline = Some(pipeline);

        // A Box allows to place multiple widgets next to each other
        // vertically or horizontally
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        vbox.pack_start(&view, true, true, 0);

        overlay.add(&vbox);
        window.add(&overlay);
    }
}
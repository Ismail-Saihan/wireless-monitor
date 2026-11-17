use wireless_monitor_lib::display::{DisplayConfig, VirtualDisplayManager, WindowsIddCxManager};

#[test]
fn test_display_config_validation_success() {
    let valid_configs = vec![
        DisplayConfig {
            width: 1920,
            height: 1080,
            refresh_rate: 60,
        },
        DisplayConfig {
            width: 2560,
            height: 1440,
            refresh_rate: 120,
        },
        DisplayConfig {
            width: 3840,
            height: 2160,
            refresh_rate: 60,
        },
        DisplayConfig {
            width: 1280,
            height: 720,
            refresh_rate: 30,
        },
    ];

    for config in valid_configs {
        assert!(
            config.validate().is_ok(),
            "Config {:?} should be valid",
            config
        );
    }
}

#[test]
fn test_display_config_validation_failures() {
    let invalid_configs = vec![
        // Zero dimensions
        (DisplayConfig { width: 0, height: 1080, refresh_rate: 60 }, "zero width"),
        (DisplayConfig { width: 1920, height: 0, refresh_rate: 60 }, "zero height"),
        (DisplayConfig { width: 1920, height: 1080, refresh_rate: 0 }, "zero refresh rate"),
        // Out of bounds width
        (DisplayConfig { width: 100, height: 1080, refresh_rate: 60 }, "width below 320"),
        (DisplayConfig { width: 10000, height: 1080, refresh_rate: 60 }, "width above 7680"),
        // Out of bounds height
        (DisplayConfig { width: 1920, height: 100, refresh_rate: 60 }, "height below 240"),
        (DisplayConfig { width: 1920, height: 5000, refresh_rate: 60 }, "height above 4320"),
        // Out of bounds refresh rate
        (DisplayConfig { width: 1920, height: 1080, refresh_rate: 5 }, "refresh rate below 10"),
        (DisplayConfig { width: 1920, height: 1080, refresh_rate: 500 }, "refresh rate above 360"),
    ];

    for (config, reason) in invalid_configs {
        assert!(
            config.validate().is_err(),
            "Config {:?} should fail validation ({})",
            config,
            reason
        );
    }
}

#[test]
fn test_create_display_and_id_tracking() {
    let mut manager = WindowsIddCxManager::new_simulated();
    assert_eq!(manager.display_count(), 0);
    assert!(manager.active_displays().is_empty());

    let config1 = DisplayConfig {
        width: 1920,
        height: 1080,
        refresh_rate: 60,
    };
    let id1 = manager
        .create_display(&config1)
        .expect("Should create display 1");
    assert!(id1 > 0);
    assert_eq!(manager.display_count(), 1);

    let config2 = DisplayConfig {
        width: 2560,
        height: 1440,
        refresh_rate: 120,
    };
    let id2 = manager
        .create_display(&config2)
        .expect("Should create display 2");
    assert!(id2 > 0);
    assert_ne!(id1, id2, "Monitor IDs must be unique");
    assert_eq!(manager.display_count(), 2);

    let active = manager.active_displays();
    assert!(active.contains(&id1));
    assert!(active.contains(&id2));

    let retrieved1 = manager.get_display_config(id1).expect("Get config 1");
    assert_eq!(retrieved1.width, 1920);
    assert_eq!(retrieved1.height, 1080);
    assert_eq!(retrieved1.refresh_rate, 60);

    let retrieved2 = manager.get_display_config(id2).expect("Get config 2");
    assert_eq!(retrieved2.width, 2560);
    assert_eq!(retrieved2.height, 1440);
    assert_eq!(retrieved2.refresh_rate, 120);
}

#[test]
fn test_create_display_rejects_invalid_config() {
    let mut manager = WindowsIddCxManager::new_simulated();
    let invalid = DisplayConfig {
        width: 0,
        height: 1080,
        refresh_rate: 60,
    };

    let result = manager.create_display(&invalid);
    assert!(result.is_err(), "create_display must reject invalid config");
    assert_eq!(manager.display_count(), 0);
}

#[test]
fn test_destroy_display_lifecycle() {
    let mut manager = WindowsIddCxManager::new_simulated();
    let config = DisplayConfig {
        width: 1920,
        height: 1080,
        refresh_rate: 60,
    };

    let id = manager.create_display(&config).expect("Create display");
    assert_eq!(manager.display_count(), 1);

    // Destroy existing display
    let destroy_res = manager.destroy_display(id);
    assert!(destroy_res.is_ok(), "destroy_display should succeed");
    assert_eq!(manager.display_count(), 0);
    assert!(manager.active_displays().is_empty());

    // Destroy already destroyed display
    let second_destroy = manager.destroy_display(id);
    assert!(
        second_destroy.is_err(),
        "Destroying non-existent display should return Err"
    );

    // Destroy non-existent ID
    let nonexistent = manager.destroy_display(9999);
    assert!(
        nonexistent.is_err(),
        "Destroying nonexistent display ID should return Err"
    );
}

#[test]
fn test_destroy_all_cleanup() {
    let mut manager = WindowsIddCxManager::new_simulated();
    let config = DisplayConfig {
        width: 1920,
        height: 1080,
        refresh_rate: 60,
    };

    let _id1 = manager.create_display(&config).unwrap();
    let _id2 = manager.create_display(&config).unwrap();
    let _id3 = manager.create_display(&config).unwrap();
    assert_eq!(manager.display_count(), 3);

    let destroy_all_res = manager.destroy_all();
    assert!(destroy_all_res.is_ok(), "destroy_all should succeed");
    assert_eq!(manager.display_count(), 0);
    assert!(manager.active_displays().is_empty());
}

#[test]
fn test_drop_safety() {
    {
        let mut manager = WindowsIddCxManager::new_simulated();
        let config = DisplayConfig {
            width: 1920,
            height: 1080,
            refresh_rate: 60,
        };
        let _id = manager.create_display(&config).unwrap();
        assert_eq!(manager.display_count(), 1);
        // manager goes out of scope here; Drop should execute destroy_all safely
    }
    // No panic, clean shutdown
}

#[test]
fn test_driver_status_and_installation_check() {
    let manager = WindowsIddCxManager::new_simulated();
    let installed = manager.is_driver_installed();
    assert!(installed.is_ok());
}

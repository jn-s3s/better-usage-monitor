//! Labels shared by the native settings menus and configuration error notices.
use super::LanguageId;

#[derive(Clone, Copy, Debug)]
pub enum SettingsText {
    Accounts,
    Monitoring,
    Taskbar,
    Preferences,
    Adaptive,
    Session,
    Weekly,
    Usage,
    Appearance,
    Recommended,
    ResetAppearance,
    System,
    Dark,
    Light,
    Colors,
    Continuous,
    Segmented,
    BarStyle,
    Standard,
    Slim,
    Thickness,
    Larger,
    TextSize,
    Alerts,
    Off,
    Remaining,
    PinLeft,
    Display,
    ClaudeLogin,
    SaveFailed,
    StartupFailed,
}

impl SettingsText {
    pub fn text(self, language: LanguageId) -> &'static str {
        // Keep this order explicit, independent of enum discriminants and ALL.
        let index = match language {
            LanguageId::English => 0,
            LanguageId::Dutch => 1,
            LanguageId::Spanish => 2,
            LanguageId::French => 3,
            LanguageId::German => 4,
            LanguageId::Japanese => 5,
            LanguageId::Korean => 6,
            LanguageId::SimplifiedChinese => 7,
            LanguageId::TraditionalChinese => 8,
            LanguageId::Russian => 9,
            LanguageId::PortugueseBrazil => 10,
        };
        let translations: [&str; 11] = match self {
            Self::Accounts => ["Accounts & services", "Accounts en diensten", "Cuentas y servicios", "Comptes et services", "Konten und Dienste", "アカウントとサービス", "계정 및 서비스", "账号与服务", "帳號與服務", "Аккаунты и сервисы", "Contas e serviços"],
            Self::Monitoring => ["Monitoring", "Bewaking", "Monitoreo", "Surveillance", "Überwachung", "監視", "모니터링", "监控", "監控", "Мониторинг", "Monitoramento"],
            Self::Taskbar => ["Taskbar", "Taakbalk", "Barra de tareas", "Barre des tâches", "Taskleiste", "タスクバー", "작업 표시줄", "任务栏", "工作列", "Панель задач", "Barra de tarefas"],
            Self::Preferences => ["Preferences", "Voorkeuren", "Preferencias", "Préférences", "Einstellungen", "環境設定", "환경 설정", "偏好设置", "偏好設定", "Настройки", "Preferências"],
            Self::Adaptive => ["Refresh every minute when remaining ≤20%", "Elke minuut vernieuwen bij ≤20% resterend", "Actualizar cada minuto si queda ≤20%", "Actualiser chaque minute si le quota restant est ≤20 %", "Bei ≤20 % Restkontingent jede Minute aktualisieren", "残量が20%以下なら毎分更新", "잔여량이 20% 이하면 매분 새로 고침", "剩余 ≤20% 时每分钟刷新", "剩餘 ≤20% 時每分鐘重新整理", "Обновлять каждую минуту при остатке ≤20%", "Atualizar a cada minuto quando restar ≤20%"],
            Self::Session => ["5-hour quota", "Quotum van 5 uur", "Cuota de 5 horas", "Quota de 5 heures", "5-Stunden-Kontingent", "5時間の利用枠", "5시간 할당량", "5 小时额度", "5 小時額度", "Квота на 5 часов", "Cota de 5 horas"],
            Self::Weekly => ["Weekly quota", "Weekquotum", "Cuota semanal", "Quota hebdomadaire", "Wochenkontingent", "週間の利用枠", "주간 할당량", "每周额度", "每週額度", "Недельная квота", "Cota semanal"],
            Self::Usage => ["Usage windows", "Quotumvensters", "Ventanas de uso", "Périodes d’utilisation", "Kontingentzeiträume", "利用枠の表示", "사용량 기간", "额度窗口", "額度視窗", "Периоды квот", "Períodos de uso"],
            Self::Appearance => ["Appearance", "Uiterlijk", "Apariencia", "Apparence", "Darstellung", "外観", "모양", "外观", "外觀", "Оформление", "Aparência"],
            Self::Recommended => ["Recommended: translucent dark taskbar", "Aanbevolen: doorschijnende donkere taakbalk", "Recomendado: barra oscura translúcida", "Recommandé : barre sombre translucide", "Empfohlen: durchscheinende dunkle Taskleiste", "推奨：半透明のダークタスクバー", "권장: 반투명 어두운 작업 표시줄", "推荐：半透明深色任务栏", "建議：半透明深色工作列", "Рекомендуется: полупрозрачная тёмная панель", "Recomendado: barra escura translúcida"],
            Self::ResetAppearance => ["Reset appearance", "Uiterlijk herstellen", "Restablecer apariencia", "Réinitialiser l’apparence", "Darstellung zurücksetzen", "外観をリセット", "모양 초기화", "恢复默认外观", "還原預設外觀", "Сбросить оформление", "Restaurar aparência"],
            Self::System => ["Follow system", "Systeem volgen", "Seguir al sistema", "Suivre le système", "System folgen", "システムに合わせる", "시스템 설정 따르기", "跟随系统", "跟隨系統", "Как в системе", "Seguir o sistema"],
            Self::Dark => ["High contrast dark", "Donker met hoog contrast", "Oscuro de alto contraste", "Sombre à contraste élevé", "Dunkel mit hohem Kontrast", "高コントラスト（ダーク）", "고대비 어둡게", "高对比深色", "高對比深色", "Тёмная, высокий контраст", "Escuro de alto contraste"],
            Self::Light => ["High contrast light", "Licht met hoog contrast", "Claro de alto contraste", "Clair à contraste élevé", "Hell mit hohem Kontrast", "高コントラスト（ライト）", "고대비 밝게", "高对比浅色", "高對比淺色", "Светлая, высокий контраст", "Claro de alto contraste"],
            Self::Colors => ["Text and colors", "Tekst en kleuren", "Texto y colores", "Texte et couleurs", "Text und Farben", "文字と色", "텍스트 및 색상", "文字与颜色", "文字與色彩", "Текст и цвета", "Texto e cores"],
            Self::Continuous => ["Continuous rounded", "Doorlopend afgerond", "Continua redondeada", "Continue arrondie", "Durchgehend abgerundet", "連続（角丸）", "연속 둥근 막대", "连续圆角", "連續圓角", "Сплошная скруглённая", "Contínua arredondada"],
            Self::Segmented => ["Segmented", "Segmenten", "Segmentada", "Segmentée", "Segmentiert", "分割", "분할", "分段", "分段", "Сегментированная", "Segmentada"],
            Self::BarStyle => ["Bar style", "Balkstijl", "Estilo de barra", "Style de barre", "Balkenstil", "バーのスタイル", "막대 스타일", "进度条样式", "進度條樣式", "Стиль полосы", "Estilo da barra"],
            Self::Standard => ["Standard", "Standaard", "Estándar", "Standard", "Standard", "標準", "표준", "标准", "標準", "Стандартный", "Padrão"],
            Self::Slim => ["Slim", "Smal", "Fina", "Fine", "Schmal", "細い", "얇게", "细", "細", "Тонкая", "Fina"],
            Self::Thickness => ["Bar thickness", "Balkdikte", "Grosor de barra", "Épaisseur de barre", "Balkendicke", "バーの太さ", "막대 두께", "进度条粗细", "進度條粗細", "Толщина полосы", "Espessura da barra"],
            Self::Larger => ["Larger", "Groter", "Más grande", "Plus grand", "Größer", "大きい", "크게", "较大", "較大", "Крупнее", "Maior"],
            Self::TextSize => ["Text size", "Tekstgrootte", "Tamaño de texto", "Taille du texte", "Textgröße", "文字サイズ", "텍스트 크기", "文字大小", "文字大小", "Размер текста", "Tamanho do texto"],
            Self::Alerts => ["Quota alerts", "Quotumwaarschuwingen", "Alertas de cuota", "Alertes de quota", "Kontingentwarnungen", "利用枠の通知", "할당량 알림", "额度提醒", "額度提醒", "Оповещения о квоте", "Alertas de cota"],
            Self::Off => ["Off", "Uit", "Desactivadas", "Désactivées", "Aus", "オフ", "끄기", "关闭", "關閉", "Выкл.", "Desativados"],
            Self::Remaining => ["{percent}% remaining", "{percent}% resterend", "{percent}% restante", "{percent} % restant", "{percent} % verbleibend", "残り{percent}%", "{percent}% 남음", "剩余 {percent}%", "剩餘 {percent}%", "Осталось {percent}%", "{percent}% restante"],
            Self::PinLeft => ["Pin to the left edge of the taskbar", "Aan linkerrand van taakbalk vastzetten", "Fijar al borde izquierdo de la barra", "Épingler au bord gauche de la barre", "Am linken Taskleistenrand anheften", "タスクバーの左端に固定", "작업 표시줄 왼쪽 가장자리에 고정", "固定在任务栏最左侧", "固定在工作列最左側", "Закрепить у левого края панели задач", "Fixar na borda esquerda da barra"],
            Self::Display => ["Display", "Beeldscherm", "Pantalla", "Écran", "Bildschirm", "ディスプレイ", "디스플레이", "显示器", "顯示器", "Монитор", "Monitor"],
            Self::ClaudeLogin => ["Claude Code (CLI login required)", "Claude Code (CLI-aanmelding vereist)", "Claude Code (requiere iniciar sesión en CLI)", "Claude Code (connexion CLI requise)", "Claude Code (CLI-Anmeldung erforderlich)", "Claude Code（CLIへのログインが必要）", "Claude Code (CLI 로그인 필요)", "Claude Code（需登录 CLI）", "Claude Code（需登入 CLI）", "Claude Code (нужен вход через CLI)", "Claude Code (login na CLI necessário)"],
            Self::SaveFailed => ["Could not save settings. Changes may be lost when the app closes. Check folder permissions and available storage, then change a setting to retry.", "Instellingen opslaan mislukt. Wijzigingen kunnen verloren gaan bij afsluiten. Controleer maprechten en vrije ruimte en wijzig een instelling om opnieuw te proberen.", "No se pudo guardar la configuración. Los cambios podrían perderse al cerrar. Revisa los permisos y el espacio disponible y cambia un ajuste para reintentar.", "Impossible d’enregistrer les paramètres. Les modifications risquent d’être perdues à la fermeture. Vérifiez les droits du dossier et l’espace disponible, puis modifiez un paramètre pour réessayer.", "Einstellungen konnten nicht gespeichert werden. Änderungen können beim Beenden verloren gehen. Prüfen Sie Ordnerrechte und freien Speicher und ändern Sie eine Einstellung, um es erneut zu versuchen.", "設定を保存できませんでした。終了すると変更が失われる可能性があります。フォルダーの権限と空き容量を確認し、設定を変更して再試行してください。", "설정을 저장할 수 없습니다. 앱을 닫으면 변경 사항이 손실될 수 있습니다. 폴더 권한과 저장 공간을 확인한 후 설정을 변경하여 다시 시도하세요.", "无法保存设置。关闭应用后更改可能丢失。请检查文件夹权限和可用空间，然后更改一项设置以重试。", "無法儲存設定。關閉應用程式後變更可能遺失。請檢查資料夾權限和可用空間，然後變更一項設定以重試。", "Не удалось сохранить настройки. При закрытии изменения могут быть потеряны. Проверьте права на папку и свободное место, затем измените настройку для повторной попытки.", "Não foi possível salvar as configurações. As alterações podem ser perdidas ao fechar. Verifique as permissões da pasta e o espaço livre e altere uma configuração para tentar novamente."],
            Self::StartupFailed => ["Could not change Start with Windows. Check your Windows account permissions and try again.", "Starten met Windows wijzigen mislukt. Controleer uw Windows-accountrechten en probeer opnieuw.", "No se pudo cambiar el inicio con Windows. Revisa los permisos de tu cuenta e inténtalo de nuevo.", "Impossible de modifier le démarrage avec Windows. Vérifiez les autorisations de votre compte et réessayez.", "Start mit Windows konnte nicht geändert werden. Prüfen Sie Ihre Windows-Kontoberechtigungen und versuchen Sie es erneut.", "Windows起動時の実行設定を変更できませんでした。アカウントの権限を確認して再試行してください。", "Windows 시작 시 실행 설정을 변경할 수 없습니다. Windows 계정 권한을 확인하고 다시 시도하세요.", "无法更改随 Windows 启动设置。请检查 Windows 账号权限并重试。", "無法變更隨 Windows 啟動設定。請檢查 Windows 帳號權限並重試。", "Не удалось изменить запуск с Windows. Проверьте права учётной записи Windows и повторите попытку.", "Não foi possível alterar o início com o Windows. Verifique as permissões da conta e tente novamente."],
        };
        translations[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_settings_label_is_present_in_every_language() {
        use SettingsText::*;
        let keys = [
            Accounts,
            Monitoring,
            Taskbar,
            Preferences,
            Adaptive,
            Session,
            Weekly,
            Usage,
            Appearance,
            Recommended,
            ResetAppearance,
            System,
            Dark,
            Light,
            Colors,
            Continuous,
            Segmented,
            BarStyle,
            Standard,
            Slim,
            Thickness,
            Larger,
            TextSize,
            Alerts,
            Off,
            Remaining,
            PinLeft,
            Display,
            ClaudeLogin,
            SaveFailed,
            StartupFailed,
        ];
        for language in LanguageId::ALL {
            for key in keys {
                let label = key.text(language);
                assert!(!label.trim().is_empty(), "{language:?}: {key:?}");
                assert!(!label.contains('\0'), "{language:?}: {key:?}");
            }
        }
    }

    #[test]
    fn remaining_templates_preserve_placeholder_for_every_language() {
        for language in LanguageId::ALL {
            assert!(SettingsText::Remaining.text(language).contains("{percent}"));
            assert!(!SettingsText::SaveFailed.text(language).is_empty());
            assert!(!SettingsText::StartupFailed.text(language).is_empty());
        }
    }
}

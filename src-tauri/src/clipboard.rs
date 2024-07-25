use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::sync::Mutex;
use tokio::time;

// 状态枚举，用户控制后台任务
#[derive(Debug, Clone, Copy, PartialEq)]
enum TaskState {
    Running,
    Paused,
}

pub struct TaskManager {
    /// 任命状态
    state: Mutex<TaskState>,
    /// 间隔时间
    duration: Mutex<Duration>,
    /// 数据库链接
    //
    app: AppHandle,
}

impl TaskManager {
    pub fn new(app: AppHandle) -> Self {
        TaskManager {
            state: Mutex::new(TaskState::Running),
            duration: Mutex::new(Duration::from_millis(500)),
            app
        }
    }
    pub async fn start(&self) {
        let mut state = self.state.lock().await;
        *state = TaskState::Running;
        drop(state); // 解锁状态
        println!("开始执行遍历任务");

        // 后台任务循环
        loop {
            let duration;
            {
                let dur = self.duration.lock().await;
                duration = *dur;
                drop(dur); // 解锁状态
            }

            {
                let state = self.state.lock().await;
                if *state != TaskState::Running {
                    break; // 如果任务状态不是 Running，则退出循环
                }
                drop(state); // 解锁状态
            }

            // TODO: 执行后台任务
            let clip_text = self.app.clipboard().read_text();
            match clip_text {
                Ok(value) => {
                    println!("当前剪切板上的内容为：{}", value);
                }
                Err(e) => {
                    println!("获取剪切板内容失败: {:?}", e);
                }
            }
            // TODO: 读取

            time::sleep(duration).await;
        }
    }

    pub async fn set_duration(&self, millis: u64) {
        let mut duration = self.duration.lock().await;
        *duration = Duration::from_millis(millis);
        drop(duration); // 解锁状态
    }

    // 暂停
    pub async fn pause(&self) {
        let mut state = self.state.lock().await;
        *state = TaskState::Paused;
        drop(state); // 解锁状态
    }

    // 开始任务或则
    pub async fn resume(&self) {
        let mut state = self.state.lock().await;
        *state = TaskState::Running;
        drop(state);
    }
}
// TODO: 1. 获取剪切板并更新数据
// 1. 如何挂载在后台
// 2. 通过时间的间隔从剪切板中循环遍历获取的内容
// 3. 获取的内容有哪些的呢
// pub async fn set_clipboard(app: AppHandle) -> Result<String, String> {
//     // app.state()
// }

// TODO: 2. 从数据库中获取剪切板数据并给予给图形

// TODO: DEMO: 测试获取剪切板数据
#[tauri::command]
pub async fn get_clipboard_text(app: AppHandle) -> Result<String, String> {
    let data = app.clipboard().read_text();
    return match data {
        Ok(data) => Ok(data),
        Err(err) => Err(err.to_string()),
    };
}

/// 描述，从数据库中获取指定id的数据库，并写入粘贴板
#[tauri::command]
pub async fn set_clipboard_text(app: AppHandle, id: i32) {

}
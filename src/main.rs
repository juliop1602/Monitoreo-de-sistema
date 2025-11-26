use sysinfo::{Networks, System, Disks};
use chrono::Local;
use rusqlite::{Connection, Result, params};
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags};//Conexiones activas

fn main() -> Result<()> {
    // Inicializar sistema y refrescar datos
    let mut sys = System::new_all();
    sys.refresh_all();

    // Conectarse a la base de datos 
    let conn = Connection::open("metrics.db")?;
    
    // Creacion de la tabla
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            cpu_uso TEXT NOT NULL,
            cpu_frecuencia TEXT NOT NULL,
            mem_fisica REAL,
            mem_swap REAL,
            interface_nombre TEXT,
            data_down TEXT NOT NULL,
            data_up TEXT NOT NULL,
            conexiones_activas INTEGER,
            disk_uso_percent REAL,
            procesos_nombres TEXT,
            procesos_cpu_uso TEXT,
            procesos_memoria TEXT
        )",
        [],
    )?;

    // Obtener datos
    //--Tiempo--
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    println!("Tiempo:{}", timestamp);
    
    //--1. Metricas de la CPU--

    let mut usages = Vec::new();
    let mut freqs = Vec::new();

    // Iterar sobre cada núcleo para obtener el uso y la fecuencia.
    for (index, cpu) in sys.cpus().iter().enumerate() {
        let uso = cpu.cpu_usage(); 
        let frecuencia = cpu.frequency(); 

        println!("Núcleo {}: Uso {:.2}%, Frecuencia {} MHz", index, uso, frecuencia);

        usages.push(format!("{:.2}", uso));
        freqs.push(frecuencia.to_string());
    }

    // Separar por comas
    let usages_str = usages.join(","); 
    let freqs_str = freqs.join(",");   

    // Temperatura *pendiente

    //--2. Metricas de la Memoria--
    let mem_fisico = sys.used_memory() * 100 / sys.total_memory() ;
    let mem_swap = sys.used_swap() * 100 / sys.total_swap() ;

  
    println!("Memoria: \n Memoria fisica {}: \n Memoria swap: {}", mem_fisico, mem_swap);

    //--Red--
    let networks = Networks::new_with_refreshed_list();

    println!("=== Network Traffic (MB) ===");

    let mut interface = Vec::new();
    let mut data_down = Vec::new();
    let mut data_up = Vec::new();
    
    for (interface_name, data) in &networks {
        let download_mb = data.total_received() as f64 / (1024.0 * 1024.0);
        let upload_mb = data.total_transmitted() as f64 / (1024.0 * 1024.0);

        interface.push(interface_name.clone());
        data_down.push(format!("{:.2}", download_mb));
        data_up.push(format!("{:.2}", upload_mb));

        println!(
            "{}: ↓ {:.2} MB | ↑ {:.2} MB",
            interface_name,
            download_mb,
            upload_mb
        );
    }

    // Convertir las listas a cadenas separadas por comas
    let interface_str = interface.join(",");
    let data_down_str = data_down.join(",");
    let data_up_str = data_up.join(",");


    // Conexiones activas
    let conexiones_activas = match get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP | ProtocolFlags::UDP
    ) {
        Ok(sockets_info) => {
            let cantidad = sockets_info.len();
            println!("Conexiones activas totales: {}", cantidad);
            cantidad as i32 
        },
        Err(e) => {
            println!("Error al obtener conexiones: {}", e);
            -1 
        },
    };

    // --Porcentaje de uso
    let disks = Disks::new_with_refreshed_list();
    let disk_usage = disks
        .iter()
        .map(|disk| disk.total_space() - disk.available_space())
        .sum::<u64>() as f32
        * 100.0
        / disks
            .iter()
            .map(|disk| disk.total_space())
            .sum::<u64>() as f32;

    println!("Disk porcentaje de uso: {:.2}", disk_usage);


    std::thread::sleep(std::time::Duration::from_millis(500));

    // Segundo refresh 
    sys.refresh_all();
    // Top 5 procesos que mas consumen recursos
    
    let mut processes: Vec<_> = sys.processes().values().collect();
    
    processes.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap());



    let mut nombres = Vec::new();
    let mut cpu_uso = Vec::new();
    let mut memoria = Vec::new();

    println!("Top 5 procesos por uso de CPU:");

    for (i, process) in processes.iter().take(5).enumerate() {
        nombres.push(process.name().to_string_lossy().to_string());
        cpu_uso.push(format!("{:.2}", process.cpu_usage()));
        memoria.push(format!("{:.2}", process.memory() as f64 / (1024.0 * 1024.0)));
        println!(
            "{}. {} (PID: {}) - {:.2}% CPU - {:.2} MB Memoria",
            i + 1,
            process.name().to_string_lossy(), 
            process.pid(),
            process.cpu_usage(),
            process.memory() as f64 / (1024.0 * 1024.0)
        );
    }

    // Separar por comas
    let nombres_str = nombres.join(",");
    let cpu_uso_str = cpu_uso.join(",");
    let memoria_str = memoria.join(",");

    conn.execute(
        "INSERT INTO metrics (timestamp, cpu_uso, cpu_frecuencia, mem_fisica, mem_swap, interface_nombre,  
                            data_down, data_up, conexiones_activas, disk_uso_percent, procesos_nombres, procesos_cpu_uso, procesos_memoria)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            timestamp,
            usages_str,
            freqs_str,
            mem_fisico,
            mem_swap,
            interface_str,
            data_down_str,
            data_up_str,
            conexiones_activas,
            disk_usage,
            nombres_str,
            cpu_uso_str,
            memoria_str
        ]
    )?;

    Ok(())
}


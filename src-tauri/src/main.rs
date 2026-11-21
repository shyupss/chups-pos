// En Windows, evita que se abra una consola junto a la ventana en el build de entrega.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    chups_pos_lib::run()
}

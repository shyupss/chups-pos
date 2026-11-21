// sqlx::migrate! embebe las migraciones al compilar, pero Cargo no sabe que dependen de la
// carpeta: sin esto, una migración nueva no entra hasta que cambie algún .rs del crate.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}

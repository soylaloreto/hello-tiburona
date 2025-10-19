#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracterror, contracttype,
    Env, Symbol, Address, Bytes,
};
use soroban_sdk::xdr::ToXdr;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NombreVacio = 1,
    NombreMuyLargo = 2,
    NoAutorizado = 3,
    NoInicializado = 4,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    ContadorSaludos,
    UltimoSaludo(Address),
    ContadorPorUsuario(Address),
}

#[contract]
pub struct HelloContract;

#[contractimpl]
impl HelloContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::NoInicializado);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::ContadorSaludos, &0u32);
        env.storage().instance().extend_ttl(100u32, 100u32);

        Ok(())
    }

    pub fn hello(
        env: Env,
        usuario: Address,
        nombre: Symbol
    ) -> Result<Symbol, Error> {
        // Rechazar símbolo vacío comparándolo con un Symbol explícito vacío
        if nombre == Symbol::new(&env, "") {
            return Err(Error::NombreVacio);
        }

        // Verificar longitud máxima usando XDR
        let bytes: Bytes = nombre.clone().to_xdr(&env);
        let len = bytes.len() as usize;
        if len > 32 {
            return Err(Error::NombreMuyLargo);
        }

        // Incrementar contador global (Instance)
        let key_contador = DataKey::ContadorSaludos;
        let contador: u32 = env.storage()
            .instance()
            .get(&key_contador)
            .unwrap_or(0u32);
        env.storage()
            .instance()
            .set(&key_contador, &(contador + 1u32));

        // Incrementar contador por usuario (Persistent)
        let user_key = DataKey::ContadorPorUsuario(usuario.clone());
        let user_count: u32 = env.storage()
            .persistent()
            .get(&user_key)
            .unwrap_or(0u32);
        env.storage()
            .persistent()
            .set(&user_key, &(user_count + 1u32));
        env.storage()
            .persistent()
            .extend_ttl(&user_key, 100u32, 100u32);

        // Guardar último saludo por usuario (Persistent)
        env.storage()
            .persistent()
            .set(&DataKey::UltimoSaludo(usuario.clone()), &nombre);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::UltimoSaludo(usuario.clone()), 100u32, 100u32);

        // Mantener TTL de instancia
        env.storage()
            .instance()
            .extend_ttl(100u32, 100u32);

        Ok(Symbol::new(&env, "Hola"))
    }

    pub fn get_contador(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ContadorSaludos)
            .unwrap_or(0u32)
    }

    pub fn get_contador_usuario(env: Env, usuario: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::ContadorPorUsuario(usuario))
            .unwrap_or(0u32)
    }

    pub fn get_ultimo_saludo(env: Env, usuario: Address) -> Option<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::UltimoSaludo(usuario))
    }

    pub fn reset_contador(env: Env, caller: Address) -> Result<(), Error> {
        let admin: Address = env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NoInicializado)?;

        if caller != admin {
            return Err(Error::NoAutorizado);
        }

        env.storage()
            .instance()
            .set(&DataKey::ContadorSaludos, &0u32);

        Ok(())
    }

    pub fn transfer_admin(
        env: Env,
        caller: Address,
        nuevo_admin: Address
    ) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NoInicializado)?;

        if caller != admin {
            return Err(Error::NoAutorizado);
        }

        env.storage()
            .instance()
            .set(&DataKey::Admin, &nuevo_admin);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::Env;
    use soroban_sdk::testutils::Address as TestAddressTrait;

    fn gen_addr(env: &Env) -> Address {
        <Address as TestAddressTrait>::generate(env)
    }

    #[test]
    fn test_hello_exitoso() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);
        let usuario = gen_addr(&env);

        env.as_contract(&contract_id, || {
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");

            let nombre = Symbol::new(&env, "Ana");
            let resultado = HelloContract::hello(env.clone(), usuario.clone(), nombre.clone())
                .expect("hello failed");
            assert_eq!(resultado, Symbol::new(&env, "Hola"));

            assert_eq!(HelloContract::get_contador(env.clone()), 1u32);
            assert_eq!(HelloContract::get_ultimo_saludo(env.clone(), usuario.clone()), Some(nombre));
            assert_eq!(HelloContract::get_contador_usuario(env.clone(), usuario.clone()), 1u32);
        });
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);

        env.as_contract(&contract_id, || {
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");
            let contador: u32 = HelloContract::get_contador(env.clone());
            assert_eq!(contador, 0u32);
        });
    }

    #[test]
    fn test_no_reinicializar() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);

        env.as_contract(&contract_id, || {
            // Primera inicialización OK
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");
            // Segunda inicialización debe devolver Err(NoInicializado)
            let res = HelloContract::initialize(env.clone(), admin.clone());
            assert_eq!(res, Err(Error::NoInicializado));
        });
    }

    #[test]
    fn test_nombre_vacio() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);
        let usuario = gen_addr(&env);

        env.as_contract(&contract_id, || {
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");

            let vacio = Symbol::new(&env, "");
            let res = HelloContract::hello(env.clone(), usuario.clone(), vacio);
            assert_eq!(res, Err(Error::NombreVacio));
        });
    }

    #[test]
    fn test_reset_solo_admin() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);
        let usuario = gen_addr(&env);

        env.as_contract(&contract_id, || {
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");

            HelloContract::hello(env.clone(), usuario.clone(), Symbol::new(&env, "Test"))
                .expect("hello failed");
            assert_eq!(HelloContract::get_contador(env.clone()), 1u32);

            HelloContract::reset_contador(env.clone(), admin.clone()).expect("reset failed");
            assert_eq!(HelloContract::get_contador(env.clone()), 0u32);
        });
    }

    #[test]
    fn test_reset_no_autorizado() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);
        let otro = gen_addr(&env);

        env.as_contract(&contract_id, || {
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");

            let res = HelloContract::reset_contador(env.clone(), otro);
            assert_eq!(res, Err(Error::NoAutorizado));
        });
    }

    #[test]
    fn test_contador_por_usuario() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);
        let usuario = gen_addr(&env);

        env.as_contract(&contract_id, || {
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");

            // inicialmente 0
            assert_eq!(HelloContract::get_contador_usuario(env.clone(), usuario.clone()), 0u32);

            // saludo 1
            HelloContract::hello(env.clone(), usuario.clone(), Symbol::new(&env, "A"))
                .expect("hello failed");
            assert_eq!(HelloContract::get_contador_usuario(env.clone(), usuario.clone()), 1u32);

            // saludo 2
            HelloContract::hello(env.clone(), usuario.clone(), Symbol::new(&env, "B"))
                .expect("hello failed");
            assert_eq!(HelloContract::get_contador_usuario(env.clone(), usuario.clone()), 2u32);

            // otro usuario no se ve afectado
            let otro = gen_addr(&env);
            assert_eq!(HelloContract::get_contador_usuario(env.clone(), otro), 0u32);
        });
    }

    #[test]
    fn test_transfer_admin() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);
        let nuevo = gen_addr(&env);
        let otro = gen_addr(&env);

        env.as_contract(&contract_id, || {
            // Inicializar con admin
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");

            // Transferir con admin actual: OK
            HelloContract::transfer_admin(env.clone(), admin.clone(), nuevo.clone())
                .expect("transfer failed");

            // Ahora solo 'nuevo' puede resetear
            let res_not_allowed = HelloContract::reset_contador(env.clone(), admin.clone());
            assert_eq!(res_not_allowed, Err(Error::NoAutorizado));

            HelloContract::reset_contador(env.clone(), nuevo.clone()).expect("reset by nuevo failed");

            // Intento de transferir por no-admin debe fallar
            let err = HelloContract::transfer_admin(env.clone(), otro.clone(), admin.clone());
            assert_eq!(err, Err(Error::NoAutorizado));
        });
    }
}
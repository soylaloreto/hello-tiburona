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
    LimiteCaracteres, // <-- agregado
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
        // establecer límite por defecto 32 caracteres
        env.storage().instance().set(&DataKey::LimiteCaracteres, &32u32);
        env.storage().instance().extend_ttl(100u32, 100u32);

        Ok(())
    }

    pub fn set_limite(
        env: Env,
        caller: Address,
        limite: u32
    ) -> Result<(), Error> {
        // comprobar que contrato esté inicializado y obtener admin
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NoInicializado)?;

        if caller != admin {
            return Err(Error::NoAutorizado);
        }

        // Guardar nuevo límite
        env.storage().instance().set(&DataKey::LimiteCaracteres, &limite);
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

        // Obtener límite (si no está presente, usar 32 como fallback)
        let limite: u32 = env.storage()
            .instance()
            .get(&DataKey::LimiteCaracteres)
            .unwrap_or(32u32);

        // Verificar longitud máxima usando XDR y el límite configurado
        let bytes: Bytes = nombre.clone().to_xdr(&env);
        let len = bytes.len() as usize;
        if (len as u32) > limite {
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
    fn test_set_limite_por_admin() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);
        let usuario = gen_addr(&env);

        env.as_contract(&contract_id, || {
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");

            // límite por defecto 32 -> saludo largo falla si >32
            let largo = "A".repeat(33);
            let res = HelloContract::hello(env.clone(), usuario.clone(), Symbol::new(&env, &largo));
            assert_eq!(res, Err(Error::NombreMuyLargo));

            // admin cambia límite a 40
            HelloContract::set_limite(env.clone(), admin.clone(), 40u32).expect("set_limite failed");

            // ahora el mismo nombre de 33 bytes debería pasar
            let res2 = HelloContract::hello(env.clone(), usuario.clone(), Symbol::new(&env, &largo));
            assert!(res2.is_ok());
        });
    }

    #[test]
    fn test_set_limite_no_autorizado() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);
        let otro = gen_addr(&env);

        env.as_contract(&contract_id, || {
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");

            let err = HelloContract::set_limite(env.clone(), otro.clone(), 10u32);
            assert_eq!(err, Err(Error::NoAutorizado));
        });
    }

    #[test]
    fn test_hello_respects_limite() {
        let env = Env::default();
        let contract_id = env.register(HelloContract, ());
        let admin = gen_addr(&env);
        let usuario = gen_addr(&env);

        env.as_contract(&contract_id, || {
            HelloContract::initialize(env.clone(), admin.clone()).expect("init fail");

            // establecer límite pequeño
            HelloContract::set_limite(env.clone(), admin.clone(), 2u32).expect("set_limite failed");

            // 3 caracteres -> debería fallar
            let res = HelloContract::hello(env.clone(), usuario.clone(), Symbol::new(&env, "ABC"));
            assert_eq!(res, Err(Error::NombreMuyLargo));

            // 2 caracteres -> ok
            let res2 = HelloContract::hello(env.clone(), usuario.clone(), Symbol::new(&env, "AB"));
            assert!(res2.is_ok());
        });
    }
}
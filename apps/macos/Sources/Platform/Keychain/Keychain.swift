import Foundation
import Security

/// Generic-password items under Wisp's service name (spec §102: secrets only in the Keychain).
public enum Keychain {
    static let service = "com.wisp.assistant"

    public enum KeychainError: Error, LocalizedError {
        case status(OSStatus)
        case invalidAccount

        public var errorDescription: String? {
            switch self {
            case .status(let s): SecCopyErrorMessageString(s, nil) as String? ?? "Keychain error \(s)"
            case .invalidAccount: "Invalid keychain account name."
            }
        }
    }

    static func validate(_ account: String) throws {
        guard !account.isEmpty, account.count <= 128,
              account.allSatisfy({ $0.isLetter || $0.isNumber || "-_.:".contains($0) }) else { throw KeychainError.invalidAccount }
    }

    private static func query(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account]
    }

    public static func set(_ secret: String, account: String) throws {
        try validate(account)
        let data = Data(secret.utf8)
        let status = SecItemUpdate(query(account) as CFDictionary, [kSecValueData as String: data] as CFDictionary)
        if status == errSecItemNotFound {
            var add = query(account)
            add[kSecValueData as String] = data
            add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            let s = SecItemAdd(add as CFDictionary, nil)
            guard s == errSecSuccess else { throw KeychainError.status(s) }
        } else if status != errSecSuccess {
            throw KeychainError.status(status)
        }
    }

    public static func get(account: String) throws -> String? {
        try validate(account)
        var q = query(account)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        let s = SecItemCopyMatching(q as CFDictionary, &out)
        if s == errSecItemNotFound { return nil }
        guard s == errSecSuccess, let data = out as? Data else { throw KeychainError.status(s) }
        return String(data: data, encoding: .utf8)
    }

    public static func delete(account: String) throws {
        try validate(account)
        let s = SecItemDelete(query(account) as CFDictionary)
        guard s == errSecSuccess || s == errSecItemNotFound else { throw KeychainError.status(s) }
    }
}

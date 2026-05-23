use crate::logger::{Logger, LogLevel};
use crate::powershell::run_powershell;
use crate::types::MemInfo;

const MEM_INFO_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$src = @'
using System;
using System.Runtime.InteropServices;

public static class WSMemInfo {
    [DllImport("ntdll.dll")]
    public static extern uint NtQuerySystemInformation(int Class, IntPtr Info, int Length, out int RetLen);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GlobalMemoryStatusEx(ref MEMORYSTATUSEX lpBuffer);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    public struct MEMORYSTATUSEX {
        public uint dwLength;
        public uint dwMemoryLoad;
        public ulong ullTotalPhys;
        public ulong ullAvailPhys;
        public ulong ullTotalPageFile;
        public ulong ullAvailPageFile;
        public ulong ullTotalVirtual;
        public ulong ullAvailVirtual;
        public ulong ullAvailExtendedVirtual;
    }

    public static string Get() {
        var ms = new MEMORYSTATUSEX();
        ms.dwLength = (uint)Marshal.SizeOf(typeof(MEMORYSTATUSEX));
        GlobalMemoryStatusEx(ref ms);

        // SystemMemoryListInformation = 80 (0x50). Размер структуры:
        // ZeroPage, FreePage, ModifiedPage, ModifiedNoWritePage, BadPage,
        // PageCountByPriority[8], RepurposedPagesByPriority[8], ModifiedPageCountPageFile
        // = 5 + 8 + 8 + 1 = 22 SIZE_T полей.
        int slots = 22;
        int sz = IntPtr.Size * slots;
        IntPtr buf = Marshal.AllocHGlobal(sz);
        ulong standby = 0;
        ulong modified = 0;
        ulong free = 0;
        try {
            int ret;
            uint status = NtQuerySystemInformation(80, buf, sz, out ret);
            if (status == 0) {
                int psz = Environment.SystemPageSize;
                long zp = Read(buf, 0);
                long fp = Read(buf, 1);
                long mp = Read(buf, 2);
                free = (ulong)(zp + fp) * (ulong)psz;
                modified = (ulong)mp * (ulong)psz;
                long s = 0;
                for (int i = 0; i < 8; i++) s += Read(buf, 5 + i);
                standby = (ulong)s * (ulong)psz;
            }
        } finally {
            Marshal.FreeHGlobal(buf);
        }
        return string.Format(
            "total={0};avail={1};standby={2};modified={3};free={4};load={5}",
            ms.ullTotalPhys, ms.ullAvailPhys, standby, modified, free, ms.dwMemoryLoad);
    }

    static long Read(IntPtr p, int idx) {
        IntPtr v = Marshal.ReadIntPtr(p, idx * IntPtr.Size);
        return v.ToInt64();
    }
}
'@
Add-Type -TypeDefinition $src -Language CSharp | Out-Null
[WSMemInfo]::Get()
"#;

const MEM_CLEAN_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$src = @'
using System;
using System.Runtime.InteropServices;

public static class WSMemClean {
    // ВАЖНО: LUID должен быть отдельной структурой из двух 32-битных полей.
    // Если объявить его как `long` внутри TOKEN_PRIVILEGES, в 64-битном
    // процессе компилятор добавит 4 байта padding между PrivilegeCount и
    // Luid (natural alignment Int64 = 8). Из-за этого LUID уезжает на
    // неверное смещение, AdjustTokenPrivileges вернёт ERROR_NOT_ALL_ASSIGNED
    // (1300), привилегия не включится, и NtSetSystemInformation упадёт с
    // STATUS_PRIVILEGE_NOT_HELD (0xC0000061).
    [StructLayout(LayoutKind.Sequential)]
    public struct LUID {
        public uint LowPart;
        public int HighPart;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct TOKEN_PRIVILEGES {
        public uint PrivilegeCount;
        public LUID Luid;
        public uint Attributes;
    }

    [DllImport("ntdll.dll")]
    public static extern uint NtSetSystemInformation(int InfoClass, IntPtr Info, int Length);

    [DllImport("advapi32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool OpenProcessToken(IntPtr ProcessHandle, uint DesiredAccess, out IntPtr TokenHandle);

    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool LookupPrivilegeValue(string lpSystemName, string lpName, out LUID lpLuid);

    [DllImport("advapi32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool AdjustTokenPrivileges(IntPtr TokenHandle, [MarshalAs(UnmanagedType.Bool)] bool DisableAllPrivileges, ref TOKEN_PRIVILEGES NewState, uint BufferLength, IntPtr PreviousState, IntPtr ReturnLength);

    [DllImport("kernel32.dll")]
    public static extern IntPtr GetCurrentProcess();

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool CloseHandle(IntPtr h);

    const uint SE_PRIVILEGE_ENABLED = 0x00000002;
    const uint TOKEN_ADJUST_PRIVILEGES = 0x0020;
    const uint TOKEN_QUERY = 0x0008;

    static bool Enable(string name) {
        IntPtr token;
        if (!OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, out token)) return false;
        try {
            LUID luid;
            if (!LookupPrivilegeValue(null, name, out luid)) return false;
            var tp = new TOKEN_PRIVILEGES {
                PrivilegeCount = 1,
                Luid = luid,
                Attributes = SE_PRIVILEGE_ENABLED
            };
            if (!AdjustTokenPrivileges(token, false, ref tp, 0, IntPtr.Zero, IntPtr.Zero)) return false;
            // Windows возвращает true даже если привилегии нет; реальный статус — в GetLastError.
            return Marshal.GetLastWin32Error() == 0;
        } finally {
            CloseHandle(token);
        }
    }

    public static uint Run(int command) {
        Enable("SeProfileSingleProcessPrivilege");
        Enable("SeIncreaseQuotaPrivilege");
        IntPtr ptr = Marshal.AllocHGlobal(4);
        try {
            Marshal.WriteInt32(ptr, command);
            // SystemMemoryListInformation = 80
            return NtSetSystemInformation(80, ptr, 4);
        } finally {
            Marshal.FreeHGlobal(ptr);
        }
    }
}
'@
Add-Type -TypeDefinition $src -Language CSharp | Out-Null
$status = [WSMemClean]::Run($Cmd)
if ($status -eq 0) {
    Write-Output "ok"
    exit 0
} else {
    Write-Output ("NtSetSystemInformation status=0x{0:X8} (0xC0000061 = STATUS_PRIVILEGE_NOT_HELD, нужны права администратора)" -f $status)
    exit 1
}
"#;

pub fn collect_mem_info(logger: &Logger) -> MemInfo {
    let (ok, out) = run_powershell(MEM_INFO_SCRIPT, logger);
    let mut info = MemInfo::default();
    if !ok {
        logger.log(
            LogLevel::Normal,
            &format!("Memory info query failed: {out}"),
        );
        return info;
    }

    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() || !line.contains('=') {
            continue;
        }
        for part in line.split(';') {
            let Some((k, v)) = part.split_once('=') else { continue };
            match k.trim() {
                "total" => info.total_bytes = v.trim().parse().unwrap_or(0),
                "avail" => info.avail_bytes = v.trim().parse().unwrap_or(0),
                "standby" => info.standby_bytes = v.trim().parse().unwrap_or(0),
                "modified" => info.modified_bytes = v.trim().parse().unwrap_or(0),
                "free" => info.free_bytes = v.trim().parse().unwrap_or(0),
                "load" => info.memory_load = v.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
        if info.total_bytes > 0 {
            break;
        }
    }
    info
}

pub fn run_mem_op(op: MemOp, logger: &Logger) -> (bool, String) {

    let cmd = op.command();
    let wrapped = format!("$Cmd = {cmd}\n{MEM_CLEAN_SCRIPT}");
    run_powershell(&wrapped, logger)
}

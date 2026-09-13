<#
Operator-only reversible privilege validation; NOT an agent backend, GUI/MCP bridge,
watchdog, installer or authorization source. No screen/input/SSH/ACL/firewall/policy edits.
Run with explicit exe SHA, PID and StartTime.ToFileTimeUtc() from fresh evidence.
prepare: prove current admin, create separate ordinary config/task, DO NOT START.
start: use the same admin PID/birth supplied to prepare; stop once, start ordinary once.
restore: supply the ordinary PID/birth returned by start. PID=0/birth=0 explicitly means
no agent is expected; restoration then requires an empty process set and listener.
Only a completed prepared.json is supported by restore. A later failed start that
left the original admin running may use its original PID/birth for restore.
Partial prepare is NOT automatically recoverable here: inspect the prepare journal,
file hash and task marker before operator cleanup; never assume restore covers it.
Journals are CreateNew operator evidence only; no agent reads or depends on them.
Any interrupted start/restore intent is NOT replayable: inspect and intervene manually.
Root must keep its short Host MCP input lease through stop and supply KillBeforeUnixMs.
This script never acquires/extends/releases that lease or claims recover_input wrote history.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('prepare','start','restore')][string]$Mode,
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9][a-z0-9-]{0,15}$')][string]$Suffix,
    [Parameter(Mandatory)][ValidateRange(0,2147483647)][int]$ExpectedPid,
    [Parameter(Mandatory)][ValidatePattern('^(0|[1-9][0-9]{0,19})$')][string]$ExpectedBirth,
    [Parameter(Mandatory)][string]$ExpectedExePath,
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedExeSha256,
    [ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedConfigSha256 = 'b6dc9b892b1ae39f681f88cb5269b0bc8d163a50e046bfc64e533026318d29b7',
    [ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedTaskXmlSha256 = '944247c6d3120901d40536f2a351e30fd83f6261c0ec31de2490b81baeccfa2b',
    [ValidateRange(1,30)][int]$TimeoutSeconds = 15,
    [long]$KillBeforeUnixMs = 0
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$Utf8 = New-Object System.Text.UTF8Encoding($false, $true)
$OriginalTask = 'RealComputerUse-Interactive-Agent'
$AgentHome = 'C:\Users\vincentadamnemessis\AppData\Local\RealComputerUse\agent-v1'
$Config = Join-Path $AgentHome 'agent.json'
$TemporaryConfig = Join-Path $AgentHome "agent.ordinary-$Suffix.json"
$TemporaryTask = "$OriginalTask-Ordinary-$Suffix"
$Prefix = Join-Path $AgentHome "validation.privilege-$Suffix"
$Port = 45128; $Address = '10.211.55.3'
$script:Stage = 'initial_checks'; $script:FailureCode = $null; $script:FailureHint = $null
$Lock = $null; $ExePin = $null; $Held = $null

function Require([bool]$Condition, [string]$Code, [string]$Hint) {
    if (-not $Condition) { $script:FailureCode=$Code; $script:FailureHint=$Hint; throw $Code }
}
function Same-Path([string]$A,[string]$B) { return [string]::Equals($A,$B,[StringComparison]::OrdinalIgnoreCase) }
function Hash-Bytes([byte[]]$Bytes) {
    $sha=[Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-','').ToLowerInvariant() } finally { $sha.Dispose() }
}
function Hash-Text([string]$Text) { return Hash-Bytes ($Utf8.GetBytes($Text)) }
function Hash-File([string]$Path) { return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Write-New([string]$Path,[string]$Text,[Security.AccessControl.FileSecurity]$Security=$null) {
    if ($null -eq $Security) { $file=New-Object IO.FileStream($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None) }
    else { $file=New-Object IO.FileStream($Path,[IO.FileMode]::CreateNew,[Security.AccessControl.FileSystemRights]::Write,[IO.FileShare]::None,4096,[IO.FileOptions]::None,$Security) }
    try { $bytes=$Utf8.GetBytes($Text); $file.Write($bytes,0,$bytes.Length); $file.Flush($true) } finally { $file.Dispose() }
}
function Save([string]$Phase,$Value) { Write-New "$Prefix.$Phase.json" (ConvertTo-Json -InputObject $Value -Depth 64) }
function Load([string]$Phase) { return [IO.File]::ReadAllText("$Prefix.$Phase.json",$Utf8) | ConvertFrom-Json }
function Canonical($Value) {
    if ($null -eq $Value) { return 'null' }
    if ($Value -is [pscustomobject]) {
        $parts=@(foreach($name in @($Value.psobject.Properties.Name | Sort-Object -CaseSensitive)) {
            (ConvertTo-Json -InputObject $name -Compress)+':'+(Canonical $Value.$name)
        }); return '{'+($parts -join ',')+'}'
    }
    if ($Value -is [Array]) { return '['+(@(foreach($v in $Value) { Canonical $v }) -join ',')+']' }
    return ConvertTo-Json -InputObject $Value -Compress
}
function File-Proof([string]$Path) {
    Require (((Get-Item -LiteralPath $Path).Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) 'reparse_asset' 'Do not follow a substituted asset.'
    return [pscustomobject]@{path=$Path;sha256=(Hash-File $Path);acl_sha256=(Hash-Text (Get-Acl -LiteralPath $Path).Sddl)}
}
function Task-Xml([string]$Name) { return [string](Export-ScheduledTask -TaskName $Name -TaskPath '\') }
function Task-Exists([string]$Name) { return @($Folder.GetTasks(1) | Where-Object { $_.Name -eq $Name }).Count -ne 0 }
function Task-Shape($Task,[string]$Configuration,[int]$Level,[string]$Sid) {
    $d=$Task.Definition
    Require ($d.Actions.Count -eq 1 -and $d.Actions.Item(1).Type -eq 0) 'task_actions_changed' 'Exactly one executable action is required.'
    Require ((Same-Path $d.Actions.Item(1).Path $ExpectedExePath) -and $d.Actions.Item(1).Arguments -ceq ('--config "'+$Configuration+'"')) 'task_action_changed' 'Executable/configuration action differs.'
    $user=$d.Principal.UserId
    if ($user -notmatch '^S-1-') { $user=(New-Object Security.Principal.NTAccount($user)).Translate([Security.Principal.SecurityIdentifier]).Value }
    Require ($user -eq $Sid -and $d.Principal.LogonType -eq 3 -and $d.Principal.RunLevel -eq $Level) 'task_principal_changed' 'Require the exact interactive SID and measured privilege task definition.'
    Require ($d.Triggers.Count -eq 0 -and $d.Settings.RestartCount -eq 0 -and $d.Settings.MultipleInstances -eq 2) 'task_replay_policy_changed' 'No triggers/restart and IgnoreNew are required.'
}
function Original-Proof {
    Require ((Hash-File $Config) -eq $ExpectedConfigSha256.ToLowerInvariant()) 'original_config_changed' 'Original config hash does not match the allocated baseline.'
    Require ((Hash-Text (Task-Xml $OriginalTask)) -eq $ExpectedTaskXmlSha256.ToLowerInvariant()) 'original_task_changed' 'Original UTF-8 task XML hash changed.'
    Task-Shape ($Folder.GetTask($OriginalTask)) $Config 1 $Cfg.platform.user_sid
}
function Original-Unchanged($Plan) {
    Original-Proof
    foreach($file in $Plan.protected_files) {
        Require ((Canonical (File-Proof $file.path)) -ceq (Canonical $file)) 'protected_asset_changed' 'Original executable/config/certificate/ACL evidence changed; no repair by guessing.'
    }
    Require ((Hash-Text ($Folder.GetTask($OriginalTask).GetSecurityDescriptor(7))) -eq $Plan.original_task_acl_sha256) 'original_task_acl_changed' 'Original task ACL changed.'
}
function Inventory([bool]$AllowTemporary) {
    foreach($task in @(Get-ScheduledTask)) { foreach($a in @($task.Actions)) {
        if ($null -ne $a.PSObject.Properties['Execute'] -and (Same-Path ([string]$a.Execute) $ExpectedExePath)) {
            Require ($task.TaskPath -eq '\' -and ($task.TaskName -eq $OriginalTask -or ($AllowTemporary -and $task.TaskName -eq $TemporaryTask))) 'other_agent_task' 'Another task can start this executable; stop for operator review.'
        }
    }}
    Require (@(Get-CimInstance Win32_Service | Where-Object { $_.PathName -and $_.PathName -like '*rcu-agent.exe*' }).Count -eq 0) 'service_watchdog_present' 'An agent service/watchdog definition requires separate review.'
}
function Agents {
    $items=@(Get-CimInstance Win32_Process -Filter "Name='rcu-agent.exe'")
    Require (@($items | Where-Object { -not $_.ExecutablePath }).Count -eq 0) 'process_path_unknown' 'An agent-named process has an unreadable path.'
    return @($items | Where-Object { Same-Path $_.ExecutablePath $ExpectedExePath })
}
function Listeners { return @(Get-NetTCPConnection -State Listen | Where-Object { $_.LocalPort -eq $Port }) }
function Listener([int]$Owner) {
    $items=@(Listeners)
    Require ($items.Count -eq 1 -and $items[0].OwningProcess -eq $Owner -and $items[0].LocalAddress -eq $Address) 'listener_mismatch' 'Require the exact PID at 10.211.55.3:45128; unknown owners are never killed.'
}
function Empty-Endpoint {
    Require (@(Agents).Count -eq 0 -and @(Listeners).Count -eq 0) 'endpoint_not_empty' 'Process/port still occupied; do not guess, kill another owner, or start a competing instance.'
}
function Probe([int]$ProcessId,[string]$Birth) {
    $base="$Prefix.probe-$([guid]::NewGuid().ToString('N'))"; $out="$base.stdout.json"; $err="$base.stderr.txt"
    Write-New $out ''; Write-New $err ''
    $probe=$null; $gone=$false; $verified=$false
    try {
        $probe=Start-Process -FilePath $ExpectedExePath -ArgumentList @('--describe-process',"$ProcessId",$Birth) -RedirectStandardOutput $out -RedirectStandardError $err -PassThru
        $null=$probe.Handle # Retained child handle, never a later PID lookup.
        if (-not $probe.WaitForExit(5000)) {
            $probe.Kill(); $gone=$probe.WaitForExit(5000)
            Require $false 'probe_timeout' 'Read-only probe timed out; no privilege or absence inferred.'
        }
        $gone=$true; $probe.WaitForExit()
        Require ($probe.ExitCode -eq 0 -and (Get-Item -LiteralPath $out).Length -le 65536 -and (Get-Item -LiteralPath $err).Length -eq 0) 'probe_failed' 'Probe exit/output is not a successful bounded native observation.'
        $v=[IO.File]::ReadAllText($out,$Utf8) | ConvertFrom-Json
        Require ($v.status -eq 'available' -and $v.pid -eq $ProcessId -and $v.creation_filetime_100ns -is [string] -and $v.creation_filetime_100ns -ceq $Birth -and (Same-Path $v.image_path $ExpectedExePath)) 'probe_identity_mismatch' 'Native process identity did not match its explicit selector.'
        Require ($v.input_sent -is [bool] -and -not $v.input_sent -and $v.permissions_changed -is [bool] -and -not $v.permissions_changed -and $v.desktop_availability_inferred -is [bool] -and -not $v.desktop_availability_inferred) 'probe_contract_mismatch' 'Probe must be read-only and must not infer desktop availability.'
        $verified=$true; return $v
    } finally {
        if ($null -ne $probe) { $probe.Dispose() }
        if ($gone -and $verified) { Remove-Item -LiteralPath $out; Remove-Item -LiteralPath $err }
    }
}
function Hold([int]$ProcessId,[string]$Birth,[string]$Configuration,[string]$Privilege,$Baseline,[bool]$SameLogon=$true) {
    $p=Get-Process -Id $ProcessId; $null=$p.Handle
    try {
        Require (-not $p.HasExited -and $p.StartTime.ToFileTimeUtc().ToString() -ceq $Birth) 'process_birth_changed' 'Expected native process no longer exists; never substitute a reused PID.'
        $w=Get-CimInstance Win32_Process -Filter "ProcessId=$ProcessId"
        $exe=[regex]::Escape($ExpectedExePath); $cfgPath=[regex]::Escape($Configuration)
        $pattern='^(?:"'+$exe+'"|'+$exe+')\s+--config\s+(?:"'+$cfgPath+'"|'+$cfgPath+')$'
        Require ((Same-Path $w.ExecutablePath $ExpectedExePath) -and $w.CommandLine.Trim() -imatch $pattern) 'process_command_changed' 'Exact executable and the single expected config argument are required.'
        $native=Probe $ProcessId $Birth; $token=$native.token_privilege
        Require ($token.elevated -is [bool] -and $token.ui_access -is [bool] -and $token.app_container -is [bool]) 'probe_token_types' 'Native token booleans must not be strings or missing values.'
        Require ($native.user_sid -eq $Cfg.platform.user_sid -and $native.session_id -gt 0 -and $native.session_id -eq $p.SessionId -and -not $token.ui_access -and -not $token.app_container) 'native_token_scope_mismatch' 'Target SID/session/token scope is not the allocated interactive user.'
        if ($Privilege -eq 'ordinary') { $valid=(-not $token.elevated -and $token.integrity_rid -ge 0x2000 -and $token.integrity_rid -lt 0x3000) }
        else { $valid=($token.elevated -and $token.integrity_rid -ge 0x3000 -and $token.integrity_rid -lt 0x4000) }
        Require ($valid -and $token.elevation_type -in @(1,2,3)) 'native_privilege_mismatch' 'Actual token privilege does not meet the requested ordinary/admin criterion.'
        if ($null -ne $Baseline) {
            Require ($native.user_sid -eq $Baseline.user_sid -and $native.session_id -eq $Baseline.session_id) 'native_session_changed' 'SID and interactive session must remain identical.'
            if ($SameLogon) { Require ((Canonical $native.authentication_id) -ceq (Canonical $Baseline.authentication_id)) 'native_logon_changed' 'Same-instance logon LUID differs from its own receipt.' }
        }
        Require ((Hash-File $ExpectedExePath) -eq $ExpectedExeSha256.ToLowerInvariant()) 'binary_changed' 'Pinned binary hash changed.'
        return [pscustomobject]@{process=$p;proof=[pscustomobject]@{pid=$ProcessId;birth=$Birth;session_id=$p.SessionId;path=$w.ExecutablePath;commandline=$w.CommandLine;exe_sha256=$ExpectedExeSha256.ToLowerInvariant();native=$native}}
    } catch { $p.Dispose(); throw }
}
function Stop-Held($Owned) {
    Require (-not $Owned.process.HasExited -and $Owned.process.StartTime.ToFileTimeUtc().ToString() -ceq $Owned.proof.birth) 'held_process_exited' 'The retained instance exited before stop; no replacement lookup.'
    Require (@(Agents).Count -eq 1) 'agent_count_changed' 'Another executable instance appeared.'
    Listener $Owned.proof.pid
    # Caller-confirmed host/guest offset is <=2s; reserve 5s conservatively.
    Require ($KillBeforeUnixMs -gt 0 -and [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()+5000 -lt $KillBeforeUnixMs) 'maintenance_lease_deadline' 'Short Host lease deadline is absent/too close/expired; no agent is killed. Reconcile and inspect without replay.'
    $Owned.process.Kill() # Same held handle; no process-tree, task-wide, name or port-owner kill.
    Require ($Owned.process.WaitForExit(5000)) 'agent_exit_timeout' 'Expected process has not exited; no further start/cleanup.'
    $until=[DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (@(Listeners).Count -ne 0 -and [DateTime]::UtcNow -lt $until) {
        Require (@(Listeners | Where-Object { $_.OwningProcess -ne $Owned.proof.pid }).Count -eq 0) 'new_listener_owner' 'Unexpected listener owner appeared; do not kill it.'
        Start-Sleep -Milliseconds 100
    }
    Empty-Endpoint
}
function Wait-Agent([string]$Configuration,[string]$Privilege,$Baseline,[string]$NotBefore,[bool]$SameLogon=$true) {
    $until=[DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $until) {
        $agents=@(Agents); $listeners=@(Listeners)
        Require ($agents.Count -le 1) 'ambiguous_agent_start' 'More than one process appeared; no selection by name/order.'
        if ($agents.Count -eq 1) {
            $p=Get-Process -Id $agents[0].ProcessId
            try { $birth=$p.StartTime.ToFileTimeUtc().ToString() } finally { $p.Dispose() }
            Require ([uint64]$birth -ge [uint64]$NotBefore) 'preexisting_agent' 'The observed instance predates this explicit start.'
            if ($listeners.Count -ne 0) {
                $owned=Hold $agents[0].ProcessId $birth $Configuration $Privilege $Baseline $SameLogon
                try { Listener $owned.proof.pid; return $owned.proof } finally { $owned.process.Dispose() }
            }
        } else { Require ($listeners.Count -eq 0) 'unknown_listener_owner' 'Port occupied without the single expected executable.' }
        Start-Sleep -Milliseconds 100
    }
    Require $false 'agent_start_timeout' 'Start requested once, but no complete process/token/listener proof arrived; inspect without replay.'
}
function Temporary-Unchanged($Plan,$Prepared) {
    Require ((Hash-File $TemporaryConfig) -eq $Plan.temporary_config_sha256) 'temporary_config_changed' 'Temporary config differs; do not delete or run it.'
    Require ((Hash-Text (Task-Xml $TemporaryTask)) -eq $Prepared.temporary_task_xml_sha256) 'temporary_task_changed' 'Temporary task differs; do not delete or run it.'
    Require ((Get-Acl -LiteralPath $TemporaryConfig).Sddl -ceq (Get-Acl -LiteralPath $Config).Sddl) 'temporary_acl_changed' 'Temporary standalone config must retain the exact protected user-only source ACL.'
    Task-Shape ($Folder.GetTask($TemporaryTask)) $TemporaryConfig 0 $Cfg.platform.user_sid
    Require ($Folder.GetTask($TemporaryTask).Definition.RegistrationInfo.Description -ceq $Plan.marker) 'temporary_owner_changed' 'Temporary task ownership marker changed.'
}
function Remove-Temporary($Plan,$Prepared) {
    Temporary-Unchanged $Plan $Prepared
    Require ($Folder.GetTask($TemporaryTask).GetInstances(0).Count -eq 0) 'temporary_task_running' 'Temporary task is not quiescent; do not unregister a running/unknown instance.'
    $Folder.DeleteTask($TemporaryTask,0)
    Require ((Hash-File $TemporaryConfig) -eq $Plan.temporary_config_sha256) 'temporary_config_changed' 'Temporary config changed before deletion.'
    Remove-Item -LiteralPath $TemporaryConfig
}

try {
    Require ($env:OS -eq 'Windows_NT') 'windows_required' 'This operator script only runs on the allocated Windows machine.'
    Require ((Same-Path $ExpectedExePath (Join-Path $AgentHome 'rcu-agent.exe')) -and [IO.Path]::GetFullPath($ExpectedExePath) -ceq $ExpectedExePath) 'unexpected_binary_path' 'Use the explicitly allocated installed binary path, not a source copy or alias.'
    Require (($ExpectedPid -eq 0) -eq ($ExpectedBirth -eq '0')) 'invalid_process_selector' 'PID and FILETIME must both be positive, or both zero for explicit absence in restore.'
    Require ($Mode -eq 'restore' -or $ExpectedPid -gt 0) 'process_selector_required' 'Prepare/start require the current admin PID and exact FILETIME.'
    $cursor=Get-Item -LiteralPath $AgentHome
    while ($null -ne $cursor) { Require (($cursor.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) 'reparse_profile' 'Profile ancestors must not be substituted.'; $cursor=$cursor.Parent }
    $Lock=New-Object IO.FileStream((Join-Path $AgentHome 'validation.privilege-roundtrip.lock'),[IO.FileMode]::CreateNew,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None,4096,[IO.FileOptions]::DeleteOnClose)
    $ExePin=New-Object IO.FileStream($ExpectedExePath,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    Require ((Hash-File $ExpectedExePath) -eq $ExpectedExeSha256.ToLowerInvariant()) 'binary_hash_mismatch' 'The explicit freshly deployed binary SHA does not match.'
    Require ((Get-Item -LiteralPath $Config).Length -le 1048576) 'config_budget' 'Original config exceeded the bounded read budget.'
    $Cfg=[IO.File]::ReadAllText($Config,$Utf8) | ConvertFrom-Json
    Require ($Cfg.platform.kind -eq 'windows' -and $Cfg.platform.privilege -eq 'admin' -and $Cfg.bind -eq "$($Address):$Port") 'original_config_scope' 'Original config must remain the allocated admin/listener configuration.'
    $Scheduler=New-Object -ComObject 'Schedule.Service'; $Scheduler.Connect(); $Folder=$Scheduler.GetFolder('\')
    Original-Proof
    if ($Mode -eq 'prepare') {
        $script:Stage='prepare_assets'
        foreach($phase in @('prepare','prepared','start-intent','task-started','ordinary','restore-intent','restored')) { Require (-not [IO.File]::Exists("$Prefix.$phase.json")) 'journal_collision' 'This suffix already has evidence; do not overwrite/replay it.' }
        Require (-not [IO.File]::Exists($TemporaryConfig) -and -not (Task-Exists $TemporaryTask)) 'temporary_collision' 'Derived task/config collision; no overwrite.'
        Inventory $false; $Held=Hold $ExpectedPid $ExpectedBirth $Config 'admin' $null
        Require (@(Agents).Count -eq 1) 'original_agent_count' 'Exactly one original executable instance required.'; Listener $ExpectedPid
        $copy=(ConvertTo-Json -InputObject $Cfg -Depth 64) | ConvertFrom-Json; $copy.platform.privilege='ordinary'
        $text=ConvertTo-Json -InputObject $copy -Depth 64; $roundtrip=$text | ConvertFrom-Json; $roundtrip.platform.privilege='admin'
        Require ((Canonical $roundtrip) -ceq (Canonical $Cfg)) 'config_roundtrip_failed' 'Only platform.privilege may change structurally.'
        $plan=[pscustomobject]@{schema='rcu.operator.privilege-roundtrip.v1';suffix=$Suffix;marker="RCU operator-only ordinary roundtrip $Suffix $([guid]::NewGuid())";original=$Held.proof;original_config_sha256=$ExpectedConfigSha256;original_task_xml_sha256=$ExpectedTaskXmlSha256;original_task_acl_sha256=(Hash-Text ($Folder.GetTask($OriginalTask).GetSecurityDescriptor(7)));temporary_config_sha256=(Hash-Text $text);protected_files=@(foreach($path in @($Config,$ExpectedExePath,$Cfg.identity.ca,$Cfg.identity.certificate,$Cfg.identity.private_key)) { File-Proof $path })}
        Save 'prepare' $plan; $security=Get-Acl -LiteralPath $Config
        Require ($security.AreAccessRulesProtected) 'source_config_acl_unprotected' 'Do not inherit or repair the source ACL.'
        Write-New $TemporaryConfig $text $security
        Require ((Get-Acl -LiteralPath $TemporaryConfig).Sddl -ceq $security.Sddl) 'new_config_acl_mismatch' 'New configuration must be born with the exact protected source ACL; no in-place repair.'
        $def=$Scheduler.NewTask(0); $def.RegistrationInfo.Description=$plan.marker
        $def.Principal.UserId=$Cfg.platform.user_sid; $def.Principal.LogonType=3; $def.Principal.RunLevel=0
        $def.Settings.Enabled=$true; $def.Settings.AllowDemandStart=$true; $def.Settings.MultipleInstances=2
        $def.Settings.RestartCount=0; $def.Settings.StartWhenAvailable=$false; $def.Settings.ExecutionTimeLimit='PT0S'
        $def.Settings.DisallowStartIfOnBatteries=$false; $def.Settings.StopIfGoingOnBatteries=$false
        $action=$def.Actions.Create(0); $action.Path=$ExpectedExePath; $action.Arguments='--config "'+$TemporaryConfig+'"'; $action.WorkingDirectory=$AgentHome
        $null=$Folder.RegisterTaskDefinition($TemporaryTask,$def,2,$Cfg.platform.user_sid,$null,3,$null) # TASK_CREATE, never CREATE_OR_UPDATE.
        $prepared=[pscustomobject]@{temporary_task_xml_sha256=(Hash-Text (Task-Xml $TemporaryTask));task=$TemporaryTask;config=$TemporaryConfig;started=$false}
        Temporary-Unchanged $plan $prepared; Original-Unchanged $plan
        Require ($Folder.GetTask($TemporaryTask).GetInstances(0).Count -eq 0 -and @(Agents).Count -eq 1) 'prepare_not_quiescent' 'An unexpected task/process appeared; prepare must not start ordinary.'
        Listener $ExpectedPid; Save 'prepared' $prepared
        $result=[pscustomobject]@{status='prepared_not_started';original=$plan.original;temporary=$prepared;config_sha256=$plan.temporary_config_sha256;journal_prefix=$Prefix}
    } else {
        $plan=Load 'prepare'; $prepared=Load 'prepared'
        Require ($plan.schema -eq 'rcu.operator.privilege-roundtrip.v1' -and $plan.suffix -ceq $Suffix -and (Same-Path $plan.original.path $ExpectedExePath) -and $plan.original.exe_sha256 -eq $ExpectedExeSha256.ToLowerInvariant() -and $plan.original_config_sha256 -eq $ExpectedConfigSha256 -and $plan.original_task_xml_sha256 -eq $ExpectedTaskXmlSha256) 'journal_scope_mismatch' 'Journal alone grants no authority; explicit path/hash/baseline must match.'
        Original-Unchanged $plan; Temporary-Unchanged $plan $prepared; Inventory $true
        if ($Mode -eq 'start') {
            $script:Stage='stop_original_and_start_once'
            Require ($ExpectedPid -eq $plan.original.pid -and $ExpectedBirth -ceq $plan.original.birth) 'original_selector_changed' 'Start requires the same exact admin instance proven in prepare.'
            $Held=Hold $ExpectedPid $ExpectedBirth $Config 'admin' $plan.original.native; Listener $ExpectedPid
            Save 'start-intent' ([pscustomobject]@{original=$Held.proof;not_before=[DateTime]::UtcNow.ToFileTimeUtc().ToString();kill_before_unix_ms=$KillBeforeUnixMs;safety_reserve_ms=5000;replay='forbidden'})
            Stop-Held $Held; $startTime=[DateTime]::UtcNow.ToFileTimeUtc().ToString()
            $running=$Folder.GetTask($TemporaryTask).Run($null)
            Save 'task-started' ([pscustomobject]@{instance_guid=$running.InstanceGuid;not_before=$startTime;task=$TemporaryTask})
            # UAC split tokens can have different LUIDs; record this new ordinary token.
            $ordinary=Wait-Agent $TemporaryConfig 'ordinary' $plan.original.native $startTime $false
            Save 'ordinary' $ordinary; Original-Unchanged $plan
            $result=[pscustomobject]@{status='ordinary_observed';ordinary=$ordinary;original_definition_unchanged=$true;native_gui_verified=$false;journal_prefix=$Prefix}
        } else {
            $script:Stage='restore_once'
            Require (-not [IO.File]::Exists("$Prefix.restore-intent.json")) 'restore_already_attempted' 'Interrupted restore is not replayed; inspect existing intent and live state manually.'
            $originalStillRunning=$ExpectedPid -eq $plan.original.pid -and $ExpectedBirth -ceq $plan.original.birth
            if ($originalStillRunning) { $Held=Hold $ExpectedPid $ExpectedBirth $Config 'admin' $plan.original.native; Listener $ExpectedPid }
            elseif ($ExpectedPid -gt 0) {
                Require ([IO.File]::Exists("$Prefix.start-intent.json")) 'ordinary_start_unproven' 'No start intent for this run; do not stop a process.'
                Require ([IO.File]::Exists("$Prefix.ordinary.json")) 'ordinary_receipt_required' 'No complete ordinary identity receipt; inspect the interrupted start manually before stopping a process.'
                $record=Load 'ordinary'
                Require ($record.pid -eq $ExpectedPid -and $record.birth -ceq $ExpectedBirth -and $record.native.user_sid -eq $plan.original.native.user_sid -and $record.native.session_id -eq $plan.original.native.session_id) 'ordinary_selector_changed' 'Restore selector/session differs from this run ordinary receipt.'
                $Held=Hold $ExpectedPid $ExpectedBirth $TemporaryConfig 'ordinary' $record.native; Listener $ExpectedPid
            } else { Empty-Endpoint }
            Save 'restore-intent' ([pscustomobject]@{selector_pid=$ExpectedPid;selector_birth=$ExpectedBirth;original_still_running=$originalStillRunning;kill_before_unix_ms=$KillBeforeUnixMs;safety_reserve_ms=5000;replay='forbidden'})
            if ($null -ne $Held -and -not $originalStillRunning) { Stop-Held $Held }
            if (-not $originalStillRunning) { Empty-Endpoint }
            Remove-Temporary $plan $prepared; Original-Unchanged $plan
            if ($originalStillRunning) { $admin=$Held.proof }
            else {
                Require ($Folder.GetTask($OriginalTask).GetInstances(0).Count -eq 0) 'original_task_not_idle' 'Original scheduler instance is not idle; do not submit another start.'
                $startTime=[DateTime]::UtcNow.ToFileTimeUtc().ToString(); $null=$Folder.GetTask($OriginalTask).Run($null)
                $admin=Wait-Agent $Config 'admin' $plan.original.native $startTime
            }
            Original-Unchanged $plan; Inventory $false
            Require (-not (Task-Exists $TemporaryTask) -and -not [IO.File]::Exists($TemporaryConfig)) 'cleanup_incomplete' 'Owned temporary assets were not completely removed.'
            $result=[pscustomobject]@{status='admin_restored';admin=$admin;original_config_sha256=$ExpectedConfigSha256;original_task_xml_sha256=$ExpectedTaskXmlSha256;temporary_assets_removed=$true;native_gui_verified=$false;journal_prefix=$Prefix}
            Save 'restored' $result
        }
    }
    $result | ConvertTo-Json -Depth 64
} catch {
    $cause=$script:FailureCode
    if ($Mode -eq 'prepare' -and [IO.File]::Exists("$Prefix.prepare.json") -and -not [IO.File]::Exists("$Prefix.prepared.json")) {
        $script:FailureCode='partial_prepare_requires_inspection'
        $script:FailureHint='No completed prepared receipt: restore does not cover partial prepare. Operator must inspect the prepare hash/task marker before cleaning only proven owned assets; no switch was attempted by prepare.'
    }
    [pscustomobject]@{status='stopped_for_manual_intervention';mode=$Mode;suffix=$Suffix;stage=$script:Stage;code=$script:FailureCode;cause_code=$cause;hint=if($script:FailureHint){$script:FailureHint}else{'Native operator command failed; inspect locally. No broader kill, cleanup, policy change or replay was attempted.'};error_type=$_.Exception.GetType().FullName;journal_prefix=$Prefix;native_gui_verified=$false} | ConvertTo-Json -Depth 8
    exit 1
} finally {
    if ($null -ne $Held) { $Held.process.Dispose() }
    if ($null -ne $ExePin) { $ExePin.Dispose() }
    if ($null -ne $Lock) { $Lock.Dispose() }
}

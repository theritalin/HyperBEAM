%%% @docThis module handles SEV-SNP attestation and verification processes.
%%% It generates attestation reports, retrieves necessary certificates,
%%% and verifies the attestation against AMD's root of trust using the
%%% snpguest and OpenSSL commands.
-module(dev_snp_attestation).
-export([generate/1, verify/1]).
-include("include/hb.hrl").

%% Define the file paths
-define(ROOT_DIR, "/tmp/tee").
-define(REQUEST_FILE, ?ROOT_DIR ++ "/request-file.txt").
-define(REPORT_FILE, ?ROOT_DIR ++ "/report.bin").
-define(CERT_CHAIN_FILE, ?ROOT_DIR ++ "/cert_chain.pem").
-define(VCEK_FILE, ?ROOT_DIR ++ "/vcek.pem").

%% Define the commands
-define(SNP_GUEST_REPORT_CMD, "snpguest report " ++ ?REPORT_FILE ++ " " ++ ?REQUEST_FILE).
-define(SNP_GUEST_CERTIFICATES_CMD, "snpguest certificates PEM " ++ ?ROOT_DIR).
-define(VERIFY_VCEK_CMD, "openssl verify --CAfile  " ++ ?CERT_CHAIN_FILE ++ " " ++ ?VCEK_FILE).
-define(VERIFY_REPORT_CMD, "snpguest verify attestation " ++ ?ROOT_DIR ++ " " ++ ?REPORT_FILE).

%% Temporarily hard-code the VCEK download command
-define(DOWNLOAD_VCEK_CMD,
    "curl --proto \'=https\' --tlsv1.2 -sSf https://kdsintf.amd.com/vcek/v1/Milan/cert_chain -o " ++ ?CERT_CHAIN_FILE
).

%% @doc Generates an attestation report and retrieves certificates.
%% Returns a binary with the attestation report and public key.
generate(Nonce) ->
    % Check if the root directory exists, and create it if not
    case filelib:is_dir(?ROOT_DIR) of
        true -> ok;
        false -> file:make_dir(?ROOT_DIR)
    end,
    % Debug: Print starting attestation generation
    ?event(starting_attestation_generation),
    % Generate request file and attestation report
    ?event(generating_request_file_with_nonce),
    generate_request_file(Nonce),
    ?event(generating_attestation_report),
    generate_attestation_report(),
    % Request certificates, download VCEK, and upload the attestation
    ?event(fetching_certificates_from_host_memory),
    fetch_certificates(),
    ?event(downloading_vcek_root_of_trust_certificate),
    download_vcek_cert(),
    % Debug: Print reading the attestation report and public key
    ?event(reading_attestation_report_and_public_key),
    % Ensure that read_file returns the binary data as expected
    {ok, ReportBin} = file:read_file(?REPORT_FILE),
    {ok, PublicKeyBin} = file:read_file(?VCEK_FILE),
    % Get sizes of the individual files (in binary)
    ReportSize = byte_size(ReportBin),
    PublicKeySize = byte_size(PublicKeyBin),
    % Debug: Print the sizes of the files
    ?event({report_size, ReportSize}),
    ?event({public_key_size, PublicKeySize}),
    % Create a binary header with the sizes and offsets
    Header = <<ReportSize:32/unit:8, PublicKeySize:32/unit:8>>,
    % Create a binary with both the report and public key data concatenated 
    % after the header
    AttestationBinary = <<Header/binary, ReportBin/binary, PublicKeyBin/binary>>,
    % Debug: Print the final binary data size
    ?event({generated_attestation_binary_size, byte_size(AttestationBinary)}),
    % Return the binary containing the attestation data
    {ok, AttestationBinary}.

%% @doc Helper to generate the request file with the padded address and nonce
generate_request_file(Nonce) ->
    RequestFile = ?REQUEST_FILE,
    NonceHex = binary_to_list(binary:encode_hex(Nonce)),
    % Debug: Print the nonce
    ?event({nonce_in_hex, NonceHex}),
    case file:write_file(RequestFile, NonceHex) of
        ok ->
            ?event({request_file_written_successfully, RequestFile}),
            ok;
        {error, Reason} ->
            ?event({failed_to_write_request_file, RequestFile, Reason}),
            {error, failed_to_write_request_file}
    end.

%% @doc Helper to generate the attestation report
generate_attestation_report() ->
    ?event({generating_snp_report, ?SNP_GUEST_REPORT_CMD}),
    case run_command(?SNP_GUEST_REPORT_CMD) of
        {ok, _} ->
            ?event({snp_report_generated_successfully}),
            ok;
        {error, Reason} ->
            ?event({failed_to_generate_snp_report, Reason}),
            {error, failed_to_generate_report}
    end.

generate_measurement(Report, Firmware, Kernel, VMSAs) ->
    ok.

%% @doc Verifies a given attestation report against the VCEK certificate
%% and AMD root of trust.
verify(AttestationBinary) ->
    % Extract the header (size info)
    <<ReportSize:32/unit:8, PublicKeySize:32/unit:8, Rest/binary>>
        = AttestationBinary,
    % Extract the individual components using the sizes from the header
    <<ReportData:ReportSize/binary, Rest1/binary>> = Rest,
    <<PublicKeyData:PublicKeySize/binary>> = Rest1,
    % Debug: Print the extracted components
    ?event({extracted_report_data, ReportData}),
    ?event({extracted_public_key_data, PublicKeyData}),
    % Write the components to temporary files (if needed for verification)
    file:write_file(?REPORT_FILE, ReportData),
    file:write_file(?VCEK_FILE, PublicKeyData),
    % Verify the VCEK certificate
    ?event({verifying_vcek_certificate, ?VERIFY_VCEK_CMD}),
    case run_command(?VERIFY_VCEK_CMD) of
        {ok, CertOutput} ->
            TrimmedOutput = string:trim(CertOutput),
            ?event({vcek_certificate_verification_output, TrimmedOutput}),
            % Compute outside the guard
            ExpectedOutput = ?VCEK_FILE ++ ": OK",
            if
                TrimmedOutput =:= ExpectedOutput ->
                    ?event({vcek_certificate_signature_verified_successfully}),
                    verify_attestation_report();
                true ->
                    ?event({vcek_signature_verification_failed, CertOutput}),
                    {error, invalid_signature}
            end;
        {error, Reason} ->
            ?event({failed_to_verify_vcek_signature, Reason}),
            {error, verification_failed}
    end.

%% @doc Verifies a given attestation report already stored in a file.
verify_attestation_report() ->
    ?event({verifying_attestation_report, ?VERIFY_REPORT_CMD}),
    case run_command(?VERIFY_REPORT_CMD) of
        {ok, Output} ->
            ?event({attestation_verification_result, Output}),
            case string:find(Output, "VEK signed the Attestation Report!", leading) of
                nomatch ->
                    ?event({attestation_verification_failed}),
                    {error, verification_failed};
                _ ->
                    ?event({attestation_verified_successfully}),
                    {ok, Output}
            end;
        {error, Reason} ->
            ?event({failed_to_verify_attestation, Reason}),
            {error, verification_failed}
    end.

%% @doc Fetches certificates from host and stores them as PEM files
fetch_certificates() ->
    ?event({fetching_sev_snp_certificates, ?SNP_GUEST_CERTIFICATES_CMD}),
    case run_command(?SNP_GUEST_CERTIFICATES_CMD) of
        {ok, _} ->
            ?event({certificates_fetched_successfully}),
            ok;
        {error, Reason} ->
            ?event({failed_to_fetch_certificates, Reason}),
            {error, failed_to_fetch_certificates}
    end.

%% @doc Downloads the VCEK root of trust certificate
download_vcek_cert() ->
    ?event({downloading_vcek_root_of_trust_certificate, ?DOWNLOAD_VCEK_CMD}),
    case run_command(?DOWNLOAD_VCEK_CMD) of
        {ok, _} ->
            ?event({vcek_root_of_trust_certificate_downloaded_successfully}),
            ok;
        {error, Reason} ->
            ?event({failed_to_download_vcek_certificate, Reason}),
            {error, failed_to_download_cert}
    end.

%%% Helpers

%% @doc Generalized function to run a shell command, hiding the stdout.
run_command(Command) ->
    ?event({running_command, Command}),
    Output = os:cmd(Command ++ " 2>&1"),
    {ok, Output}.
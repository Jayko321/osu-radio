using System.Text.Json;
using Realms;
using Realms.Logging;
using Realms.Schema;

return ProgramMain(args);

static int ProgramMain(string[] args)
{
    bool printSchema;
    string realmPath;

    switch (args)
    {
        case [var path]:
            printSchema = false;
            realmPath = path;
            break;

        case ["--schema", var path]:
            printSchema = true;
            realmPath = path;
            break;

        default:
            Console.Error.WriteLine("Usage: osu-lazer-realm-parser [--schema] <path-to-client.realm>");
            return 2;
    }

    if (!File.Exists(realmPath))
    {
        Console.Error.WriteLine($"Realm file does not exist: {realmPath}");
        return 3;
    }

    try
    {
        RealmLogger.Default = RealmLogger.Null;

        var configuration = new RealmConfiguration(Path.GetFullPath(realmPath))
        {
            IsDynamic = true,
            IsReadOnly = true,
        };

        using var realm = Realm.GetInstance(configuration);

        if (printSchema)
            PrintSchema(realm);
        else
            ExportBeatmapSets(realm, Path.GetDirectoryName(configuration.DatabasePath)!);

        return 0;
    }
    catch (Exception error)
    {
        Console.Error.WriteLine($"Failed to read Realm '{realmPath}': {error.Message}");
        return 4;
    }
}

static void PrintSchema(Realm realm)
{
    foreach (var objectSchema in realm.Schema.OrderBy(schema => schema.Name, StringComparer.Ordinal))
    {
        WriteJson(new
        {
            type = "schema",
            name = objectSchema.Name,
            objectType = objectSchema.BaseType.ToString(),
            fields = objectSchema.Select(property => new
            {
                name = property.Name,
                managedName = property.ManagedName,
                propertyType = GetPropertyType(property.Type),
                nullable = property.Type.HasFlag(PropertyType.Nullable),
                collection = GetCollectionType(property.Type),
                objectType = property.ObjectType,
                linkOriginProperty = property.LinkOriginPropertyName,
                primaryKey = property.IsPrimaryKey,
                indexType = property.IndexType.ToString(),
            }),
        });
    }
}

static string GetPropertyType(PropertyType propertyType)
{
    return (propertyType & ~PropertyType.Flags).ToString();
}

static string? GetCollectionType(PropertyType propertyType)
{
    if (propertyType.HasFlag(PropertyType.Array))
        return "list";
    if (propertyType.HasFlag(PropertyType.Set))
        return "set";
    if (propertyType.HasFlag(PropertyType.Dictionary))
        return "dictionary";

    return null;
}

static void ExportBeatmapSets(Realm realm, string lazerRoot)
{
    if (!realm.Schema.Any(schema => schema.Name == "BeatmapSet"))
        throw new InvalidDataException("Realm schema does not contain the expected 'BeatmapSet' object type");

    foreach (var beatmapSet in realm.DynamicApi.All("BeatmapSet"))
    {
        WriteJson(new
        {
            source = "Lazer",
            online_id = GetInteger(beatmapSet, "OnlineID"),
            hash = GetString(beatmapSet, "Hash"),
            files = GetNamedFileUsages(beatmapSet, lazerRoot).ToArray(),
            beatmaps = GetObjectList(beatmapSet, "Beatmaps").Select(ToBeatmap).ToArray(),
        });
    }
}

static object ToBeatmap(IRealmObjectBase beatmap)
{
    var metadata = GetObject(beatmap, "Metadata");

    return new
    {
        difficulty_name = GetString(beatmap, "DifficultyName"),
        bpm = GetDouble(beatmap, "BPM"),
        hash = GetString(beatmap, "Hash"),
        metadata = metadata is null ? null : new
        {
            title = GetString(metadata, "Title"),
            title_unicode = GetString(metadata, "TitleUnicode"),
            artist = GetString(metadata, "Artist"),
            artist_unicode = GetString(metadata, "ArtistUnicode"),
            author = ToUser(GetObject(metadata, "Author")),
            source = GetString(metadata, "Source"),
            tags = GetString(metadata, "Tags"),
            user_tags = GetStringList(metadata, "UserTags").ToArray(),
            preview_time = GetInteger(metadata, "PreviewTime"),
            audio_file = GetString(metadata, "AudioFile"),
            background_file = GetString(metadata, "BackgroundFile"),
        },
    };
}

static object? ToUser(IRealmObjectBase? user)
{
    if (user is null)
        return null;

    return new
    {
        online_id = GetInteger(user, "OnlineID"),
        username = GetString(user, "Username"),
        country_code = GetString(user, "CountryCode"),
    };
}

static IEnumerable<object> GetNamedFileUsages(IRealmObjectBase beatmapSet, string lazerRoot)
{
    foreach (var usage in GetObjectList(beatmapSet, "Files"))
    {
        var file = GetObject(usage, "File");
        var hash = file is null ? null : GetString(file, "Hash");

        yield return new
        {
            filename = GetString(usage, "Filename"),
            file = file is null ? null : new
            {
                hash,
                resolved_path = GetFilePath(lazerRoot, hash),
            },
        };
    }
}

static string? GetFilePath(string lazerRoot, string? hash)
{
    if (string.IsNullOrEmpty(hash) || hash.Length < 2)
        return null;

    return Path.Combine(lazerRoot, "files", hash[..1], hash[..2], hash);
}

static IRealmObjectBase? GetObject(IRealmObjectBase value, string propertyName)
{
    try
    {
        return value.DynamicApi.Get<IRealmObjectBase?>(propertyName);
    }
    catch (MissingMemberException)
    {
        return null;
    }
}

static string? GetString(IRealmObjectBase value, string propertyName)
{
    try
    {
        return value.DynamicApi.Get<string?>(propertyName);
    }
    catch (MissingMemberException)
    {
        return null;
    }
}

static IEnumerable<IRealmObjectBase> GetObjectList(IRealmObjectBase value, string propertyName)
{
    try
    {
        return value.DynamicApi.GetList<IRealmObjectBase>(propertyName);
    }
    catch (MissingMemberException)
    {
        return Enumerable.Empty<IRealmObjectBase>();
    }
}

static IEnumerable<string> GetStringList(IRealmObjectBase value, string propertyName)
{
    try
    {
        return value.DynamicApi.GetList<string>(propertyName);
    }
    catch (MissingMemberException)
    {
        return Enumerable.Empty<string>();
    }
}

static int? GetInteger(IRealmObjectBase value, string propertyName)
{
    return GetRealmValue<long>(value, propertyName) is { } longValue
        ? ToInt32(longValue)
        : GetRealmValue<int>(value, propertyName);
}

static double? GetDouble(IRealmObjectBase value, string propertyName)
{
    return GetRealmValue<double>(value, propertyName)
        ?? GetRealmValue<float>(value, propertyName)
        ?? GetRealmValue<long>(value, propertyName)
        ?? GetRealmValue<int>(value, propertyName);
}

static T? GetRealmValue<T>(IRealmObjectBase value, string propertyName)
    where T : struct
{
    try
    {
        return value.DynamicApi.Get<T>(propertyName);
    }
    catch (MissingMemberException)
    {
        return null;
    }
    catch (InvalidCastException)
    {
        return null;
    }
}

static int? ToInt32(long value)
{
    try
    {
        return checked((int)value);
    }
    catch (OverflowException)
    {
        return null;
    }
}

static void WriteJson<T>(T value)
{
    Console.Out.WriteLine(JsonSerializer.Serialize(value));
}
